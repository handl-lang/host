//! Record (struct/union) declarations → `Record`.

use clang_sys::*;
use handl_plugin_sdkgen::{Field, Record};

use crate::cx::{children_of_kind, cursor_name, doc_comment, is_anonymous_decl, record_name};
use crate::map::map_field_type;
use crate::parse::ParseCtx;

pub(crate) fn parse_record(cursor: CXCursor, _ctx: &ParseCtx) -> Result<Record, String> {
    if is_anonymous_decl(cursor) {
        return Err("anonymous struct is not supported".into());
    }
    let name = cursor_name(cursor);
    let doc = doc_comment(cursor);
    let ty = unsafe { clang_getCursorType(cursor) };
    let size = unsafe { clang_Type_getSizeOf(ty) };
    let align = unsafe { clang_Type_getAlignOf(ty) };
    if size < 0 || align < 0 {
        return Err(format!("struct {name} has incomplete layout"));
    }

    let mut fields = Vec::new();
    for f in children_of_kind(cursor, CXCursor_FieldDecl) {
        match parse_field(f) {
            Ok(field) => fields.push(field),
            Err(e) => return Err(format!("struct {name}: {e}")),
        }
    }

    Ok(Record {
        name,
        fields,
        size: size as usize,
        align: align as usize,
        doc,
    })
}

fn parse_field(cursor: CXCursor) -> Result<Field, String> {
    let name = cursor_name(cursor);
    if unsafe { clang_Cursor_isBitField(cursor) } != 0 {
        // Bitfield widths don't map onto the descriptor vocabulary; the
        // byte-offset/size below would silently misstate the ABI.
        return Err(format!("field {name} is a bitfield"));
    }
    let ty = unsafe { clang_getCursorType(cursor) };
    let offset_bits = unsafe { clang_Cursor_getOffsetOfField(cursor) };
    let size = unsafe { clang_Type_getSizeOf(ty) };
    if size < 0 {
        return Err(format!("field {name} has unknown size"));
    }
    let canonical = unsafe { clang_getCanonicalType(ty) };
    let (port, descriptor, ptr_depth) = match canonical.kind {
        // Fixed-size inline array `T[N]` → `arr:N:elem` — byte-wide
        // elements surface as Bytes, wider ones as [Int].
        CXType_ConstantArray => {
            let elem = unsafe { clang_getArrayElementType(canonical) };
            let n = unsafe { clang_getArraySize(canonical) };
            let esz = unsafe { clang_Type_getSizeOf(elem) };
            if esz <= 0 {
                return Err(format!("field {name} has unknown element size"));
            }
            let (_, edesc) = map_field_type(elem)
                .map_err(|e| format!("field {name}: array element: {e}"))?;
            let port = if esz == 1 { "Bytes" } else { "[Int]" };
            (port.to_string(), format!("arr:{n}:{edesc}"), 0)
        }
        // Any pointer field → raw pointer slot. The marshaller passes the
        // address through; buffers/nesting need a per-function override.
        CXType_Pointer => ("Handle".to_string(), "ptr".to_string(), 1),
        // Inline nested struct → `rec:name`, expanded to `rec:{...}` when
        // the nested record is also known.
        CXType_Record => {
            let rn = record_name(canonical).map_err(|e| format!("field {name}: {e}"))?;
            (rn.clone(), format!("rec:{rn}"), 0)
        }
        _ => {
            let (p, d) = map_field_type(ty)?;
            (p, d, 0)
        }
    };
    Ok(Field {
        name,
        port,
        descriptor,
        offset: (offset_bits / 8) as usize,
        size: size as usize,
        ptr_depth,
        is_const: unsafe { clang_isConstQualifiedType(ty) != 0 },
        doc: doc_comment(cursor),
    })
}
