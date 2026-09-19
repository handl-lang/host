//! Function declarations → `Function`.

use clang_sys::*;
use handl_plugin_sdkgen::{Arg, Function};
use heck::ToUpperCamelCase;

use crate::cx::{cursor_name, doc_comment, type_spelling};
use crate::map::{map_arg_with_peek, map_return_type};
use crate::parse::ParseCtx;

pub(crate) fn parse_function(cursor: CXCursor, ctx: &ParseCtx) -> Result<Function, String> {
    let name = cursor_name(cursor);
    if name.is_empty() {
        return Err("unnamed function".into());
    }
    let doc = doc_comment(cursor);
    let port_name = name.to_upper_camel_case();
    let c_decl = type_spelling(unsafe { clang_getCursorType(cursor) });

    let n_args = unsafe { clang_Cursor_getNumArguments(cursor) };
    if n_args < 0 {
        return Err(format!("function {name} argument count unknown"));
    }
    let n = n_args as usize;
    let mut cursors = Vec::with_capacity(n);
    for i in 0..n {
        cursors.push(unsafe { clang_Cursor_getArgument(cursor, i as std::ffi::c_uint) });
    }

    let ret_type = unsafe { clang_getCursorResultType(cursor) };
    let variadic = unsafe { clang_isFunctionTypeVariadic(clang_getCursorType(cursor)) != 0 };

    let mut port_args = Vec::new();
    let mut arg_descriptors = Vec::new();
    let mut out_struct: Option<String> = None;

    let mut i = 0;
    while i < cursors.len() {
        let arg = cursors[i];
        let arg_name = cursor_name(arg);
        let arg_ty = unsafe { clang_getCursorType(arg) };
        if arg_ty.kind == CXType_Void {
            // `int f(void)` — the void placeholder is not a real argument.
            i += 1;
            continue;
        }
        let next = cursors.get(i + 1).copied();

        let info = map_arg_with_peek(arg_ty, next, ctx.pointer_width, variadic)?;
        if info.is_size_pair {
            // The pointer+length pair collapses to one handl argument
            // (Bytes) whose descriptor expands to two C args.
            if info.consumes {
                port_args.push(Arg {
                    name: arg_name,
                    port: info.port,
                });
            }
            arg_descriptors.push(info.descriptor);
            i += 2;
            continue;
        }

        if info.is_out_struct {
            match out_struct_descriptor(&info.record_name, ctx) {
                Ok(record) => {
                    if out_struct.is_some() {
                        return Err(format!("function {name} has multiple out-struct params"));
                    }
                    arg_descriptors.push(record);
                    out_struct = info.record_name.clone();
                }
                Err(_) => {
                    // Unknown or incomplete record (opaque SSL*, FILE*, ...):
                    // it is a handle the callee dereferences, not a caller-
                    // allocated out-param — pass the pointer through.
                    port_args.push(Arg {
                        name: arg_name,
                        port: "Handle".into(),
                    });
                    arg_descriptors.push("ptr".into());
                }
            }
            i += 1;
            continue;
        }

        if info.consumes {
            port_args.push(Arg {
                name: arg_name,
                port: info.port,
            });
        }
        arg_descriptors.push(info.descriptor);
        i += 1;
    }

    let (ret_port, ret_descriptor) = map_return_type(ret_type, ctx.pointer_width)?;
    let return_port = out_struct.clone().unwrap_or(ret_port);

    Ok(Function {
        name,
        port_name,
        port_args,
        arg_descriptors,
        return_port,
        ret_descriptor,
        out_struct,
        variadic,
        c_decl,
        doc,
    })
}

/// Render `out_struct:{field:desc,...}` from a previously parsed record.
fn out_struct_descriptor(name: &Option<String>, ctx: &ParseCtx) -> Result<String, String> {
    let name = name.as_ref().ok_or("missing out-struct name")?;
    let rec = ctx
        .records
        .borrow()
        .get(name)
        .cloned()
        .ok_or_else(|| format!("unknown out-struct record: {name}"))?;
    let mut desc = String::new();
    desc.push_str("out_struct:{");
    for (i, f) in rec.fields.iter().enumerate() {
        if i > 0 {
            desc.push(',');
        }
        desc.push_str(&f.name);
        desc.push(':');
        desc.push_str(&f.descriptor);
    }
    desc.push('}');
    Ok(desc)
}
