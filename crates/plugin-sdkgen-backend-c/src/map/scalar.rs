//! Scalar type mapping: function args, record fields, return types.

use clang_sys::*;

use crate::cx::type_spelling;
use crate::map::pred::{is_char, is_void};

/// C typedef spelling → (`types/c/<header>` port alias, module file).
///
/// The port type carries the header's vocabulary while the descriptor
/// keeps the real ABI width. Each alias lives in the `types/c` submodule
/// mirroring the C header that declares the typedef (`sys/socket.h` →
/// `types/c/socket`). Platform-variable types (`long`, `off_t` on
/// 32-bit) stay lowered.
const CTYPES: &[(&str, &str, &str)] = &[
    // stdint.h — fixed width on every target.
    ("uint8_t", "Uint8", "types/c/stdint"),
    ("uint16_t", "Uint16", "types/c/stdint"),
    ("uint32_t", "Uint32", "types/c/stdint"),
    ("uint64_t", "Uint64", "types/c/stdint"),
    ("int8_t", "Int8", "types/c/stdint"),
    ("int16_t", "Int16", "types/c/stdint"),
    ("int32_t", "Int32", "types/c/stdint"),
    ("int64_t", "Int64", "types/c/stdint"),
    // stddef.h
    ("size_t", "Size", "types/c/stddef"),
    // sys/types.h
    ("ssize_t", "SSize", "types/c/sys"),
    ("pid_t", "Pid", "types/c/sys"),
    ("uid_t", "Uid", "types/c/sys"),
    ("gid_t", "Gid", "types/c/sys"),
    ("mode_t", "Mode", "types/c/sys"),
    ("off_t", "Off", "types/c/sys"),
    // time.h
    ("time_t", "Time", "types/c/time"),
    ("clockid_t", "ClockId", "types/c/time"),
    // sys/socket.h
    ("socklen_t", "Socklen", "types/c/socket"),
    ("sa_family_t", "SaFamily", "types/c/socket"),
    // netinet/in.h
    ("in_addr_t", "InAddr", "types/c/netinet"),
    ("in_port_t", "InPort", "types/c/netinet"),
];

/// C typedef spelling → `types/c` port alias.
pub(crate) fn ctypes_port(spelling: &str) -> Option<&'static str> {
    CTYPES
        .iter()
        .find(|(s, ..)| *s == spelling)
        .map(|(.., a, _)| *a)
}

/// Port type name → the `types/c` submodule that exports it (the codomain
/// of [`ctypes_port`]) — used to collect `from "types/c/<h>" import {...}`.
pub(crate) fn ctypes_module(port: &str) -> Option<&'static str> {
    CTYPES
        .iter()
        .find(|(.., a, _)| *a == port)
        .map(|(.., m)| *m)
}

/// Map a scalar/typedef C type to a `(port, descriptor)` pair.
pub(crate) fn map_arg_type(ty: CXType, pointer_width: usize) -> Result<(String, String), String> {
    let spelling = type_spelling(ty);
    let alias = ctypes_port(&spelling);
    if spelling == "size_t" {
        return Ok((
            alias.unwrap_or("Nat").into(),
            if pointer_width == 32 { "i32" } else { "i64" }.into(),
        ));
    }
    if spelling == "ssize_t" {
        return Ok((
            alias.unwrap_or("Int").into(),
            if pointer_width == 32 { "i32" } else { "i64" }.into(),
        ));
    }
    if ty.kind == CXType_Typedef {
        // Named typedefs get their ctypes alias; the descriptor comes
        // from the canonical type's real width.
        let (mut port, descriptor) =
            map_arg_type(unsafe { clang_getCanonicalType(ty) }, pointer_width)?;
        if let Some(a) = alias {
            port = a.into();
        }
        return Ok((port, descriptor));
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
        _ => Err(format!("unsupported argument type: {}", type_spelling(ty))),
    }
}

/// Map a record field's C type to a `(port, descriptor)` pair.
pub(crate) fn map_field_type(ty: CXType, _size: usize) -> Result<(String, String), String> {
    let spelling = type_spelling(ty);
    let alias = ctypes_port(&spelling);
    if spelling == "size_t" {
        return Ok((alias.unwrap_or("Nat").into(), "usize".into()));
    }
    if spelling == "ssize_t" {
        return Ok((alias.unwrap_or("Int").into(), "isize".into()));
    }
    if ty.kind == CXType_Typedef {
        let (mut port, descriptor) = map_field_type(unsafe { clang_getCanonicalType(ty) }, _size)?;
        if let Some(a) = alias {
            port = a.into();
        }
        return Ok((port, descriptor));
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
    // `ty`, not canonical — the typedef spelling carries ctypes aliases.
    map_arg_type(ty, pointer_width)
}
