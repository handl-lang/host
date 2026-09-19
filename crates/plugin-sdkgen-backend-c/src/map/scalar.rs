//! Scalar type mapping: function args, record fields, return types.

use clang_sys::*;

use crate::cx::type_spelling;
use crate::map::pred::{is_char, is_void};

/// Map a scalar/typedef C type to a `(port, descriptor)` pair.
pub(crate) fn map_arg_type(ty: CXType, pointer_width: usize) -> Result<(String, String), String> {
    let spelling = type_spelling(ty);
    if spelling == "size_t" {
        return Ok((
            "Nat".into(),
            if pointer_width == 32 { "i32" } else { "i64" }.into(),
        ));
    }
    if spelling == "ssize_t" {
        return Ok((
            "Int".into(),
            if pointer_width == 32 { "i32" } else { "i64" }.into(),
        ));
    }

    match ty.kind {
        // Sub-32-bit integers are widened to i32: on the supported ABIs
        // they occupy a full-width argument slot and the callee reads the
        // low bits. (`float` stays unsupported — an f64 descriptor would
        // put double bits in the xmm slot.)
        CXType_Bool | CXType_SChar | CXType_Char_S | CXType_Short => {
            Ok(("Int".into(), "i32".into()))
        }
        CXType_UChar | CXType_Char_U | CXType_UShort => Ok(("Nat".into(), "i32".into())),
        CXType_Int => Ok(("Int".into(), "i32".into())),
        CXType_Long | CXType_LongLong => Ok(("Int".into(), "i64".into())),
        CXType_UInt | CXType_ULong | CXType_ULongLong => {
            // unsigned 32/64-bit; marshal as i32/i64
            let size = unsafe { clang_Type_getSizeOf(ty) };
            if size == 4 {
                Ok(("Nat".into(), "i32".into()))
            } else {
                Ok(("Nat".into(), "i64".into()))
            }
        }
        CXType_Double => Ok(("Float".into(), "f64".into())),
        CXType_Enum => Ok(("Int".into(), "i32".into())),
        CXType_Typedef => map_arg_type(unsafe { clang_getCanonicalType(ty) }, pointer_width),
        _ => Err(format!("unsupported argument type: {}", type_spelling(ty))),
    }
}

/// Map a record field's C type to a `(port, descriptor)` pair.
pub(crate) fn map_field_type(ty: CXType, _size: usize) -> Result<(String, String), String> {
    let spelling = type_spelling(ty);
    if spelling == "size_t" {
        return Ok(("Nat".into(), "usize".into()));
    }
    if spelling == "ssize_t" {
        return Ok(("Int".into(), "isize".into()));
    }

    match ty.kind {
        CXType_SChar | CXType_Char_S => Ok(("Int".into(), "i8".into())),
        CXType_UChar | CXType_Char_U => Ok(("Nat".into(), "u8".into())),
        CXType_Short => Ok(("Int".into(), "i16".into())),
        CXType_UShort => Ok(("Nat".into(), "u16".into())),
        CXType_Int => Ok(("Int".into(), "i32".into())),
        CXType_UInt => Ok(("Nat".into(), "u32".into())),
        CXType_Long | CXType_LongLong => Ok(("Int".into(), "i64".into())),
        CXType_ULong | CXType_ULongLong => Ok(("Nat".into(), "u64".into())),
        CXType_Double => Ok(("Float".into(), "f64".into())),
        CXType_Enum => Ok(("Int".into(), "i32".into())),
        CXType_Typedef => map_field_type(unsafe { clang_getCanonicalType(ty) }, _size),
        _ => Err(format!("unsupported field type: {}", type_spelling(ty))),
    }
}

/// Map a function's return type to a `(port, descriptor)` pair.
pub(crate) fn map_return_type(
    ty: CXType,
    pointer_width: usize,
) -> Result<(String, String), String> {
    if ty.kind == CXType_Void {
        return Ok(("()".into(), "void".into()));
    }
    if ty.kind == CXType_Pointer {
        let pointee = unsafe { clang_getPointeeType(ty) };
        let pointee_canonical = unsafe { clang_getCanonicalType(pointee) };
        if is_void(pointee_canonical) {
            return Ok(("Handle".into(), "handle".into()));
        }
        if is_char(pointee_canonical) {
            // Returned C strings are copied out by the marshaller; the C
            // side must own the storage.
            return Ok(("String".into(), "cstr".into()));
        }
    }
    let (port, descriptor) = map_arg_type(unsafe { clang_getCanonicalType(ty) }, pointer_width)?;
    Ok((port, descriptor))
}
