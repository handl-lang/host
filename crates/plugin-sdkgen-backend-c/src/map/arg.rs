//! Function-argument mapping, including `(ptr, len)` pair collapse.

use clang_sys::*;

use crate::cx::{cursor_name, record_name, type_spelling};
use crate::map::pred::{is_char, is_len_like, is_size_t, is_void};
use crate::map::scalar::{ctypes_port, map_arg_type};

/// The mapping decision for one C function argument.
pub(crate) struct ArgInfo {
    /// Handl port type (e.g. `Int`, `Bytes`, `Handle`, a record name).
    pub port: String,
    /// Marshalling descriptor (e.g. `i32`, `cstr`, `in_buf`).
    pub descriptor: String,
    /// Whether the argument consumes a handl argument at all
    /// (`out_struct` out-params consume none).
    pub consumes: bool,
    /// Whether this is a `struct T*` out-param resolved via `out_struct`.
    pub is_out_struct: bool,
    /// Whether this is a `const struct T*` in-param resolved via
    /// `in_struct` (caller passes a record; the marshaller builds it).
    pub is_in_struct: bool,
    /// The record name for `out_struct` arguments.
    pub record_name: Option<String>,
    /// Whether this argument is the pointer half of a `(ptr, len)` pair —
    /// the following argument is skipped as the descriptor supplies it.
    pub is_size_pair: bool,
}

impl ArgInfo {
    fn scalar(port: &str, descriptor: &str) -> Self {
        Self {
            port: port.into(),
            descriptor: descriptor.into(),
            consumes: true,
            is_out_struct: false,
            is_in_struct: false,
            record_name: None,
            is_size_pair: false,
        }
    }

    fn size_pair(descriptor: &str) -> Self {
        Self {
            port: "Bytes".into(),
            descriptor: descriptor.into(),
            consumes: true,
            is_out_struct: false,
            is_in_struct: false,
            record_name: None,
            is_size_pair: true,
        }
    }
}

/// Map one function argument, optionally peeking at the next argument to
/// detect `(ptr, len)` pairs.
pub(crate) fn map_arg_with_peek(
    ty: CXType,
    next: Option<CXCursor>,
    pointer_width: usize,
    variadic: bool,
) -> Result<ArgInfo, String> {
    let original = type_spelling(ty);
    let canonical = unsafe { clang_getCanonicalType(ty) };

    // size_t / ssize_t — pointer-width scalars carrying ctypes aliases.
    if original == "size_t" || original == "ssize_t" {
        let port =
            ctypes_port(&original).unwrap_or(if original == "size_t" { "Nat" } else { "Int" });
        let desc = if pointer_width == 32 { "i32" } else { "i64" };
        return Ok(ArgInfo::scalar(port, desc));
    }

    // A trailing size-typed or length-named argument is treated as the
    // length of the preceding pointer (the C `(ptr, len)` convention). For
    // variadic functions only `size_t` counts — otherwise e.g.
    // `open(const char*, int, ...)` would misread `flags` as a length.
    let next_is_len = next
        .map(|nc| {
            let next_ty = unsafe { clang_getCursorType(nc) };
            is_size_t(next_ty) || (!variadic && is_len_like(next_ty, &cursor_name(nc)))
        })
        .unwrap_or(false);

    if canonical.kind == CXType_Pointer {
        let pointee = unsafe { clang_getPointeeType(canonical) };
        let const_pointee = unsafe { clang_isConstQualifiedType(pointee) } != 0;
        let pointee_canonical = unsafe { clang_getCanonicalType(pointee) };

        // const void* + length -> in_buf
        if const_pointee && is_void(pointee_canonical) && next_is_len {
            return Ok(ArgInfo::size_pair("in_buf"));
        }
        if const_pointee && is_char(pointee_canonical) {
            if next_is_len {
                return Ok(ArgInfo::size_pair("cbytes"));
            }
            // Fall back to cstr if no size is present.
            return Ok(ArgInfo::scalar("String", "cstr"));
        }

        // const struct T* + length -> in_buf (e.g. sockaddr*, socklen_t)
        if const_pointee && pointee_canonical.kind == CXType_Record && next_is_len {
            return Ok(ArgInfo::size_pair("in_buf"));
        }

        // char* (NUL string); a mutable char* followed by a length is an
        // out-buffer the descriptor set cannot express — reject it rather
        // than emit a cstr that would silently drop the written data.
        if is_char(pointee_canonical) {
            if next_is_len {
                return Err(format!("unsupported mutable buffer: {}", type_spelling(ty)));
            }
            return Ok(ArgInfo::scalar("String", "cstr"));
        }

        // void* (opaque handle / ptr)
        if is_void(pointee_canonical) && !const_pointee {
            return Ok(ArgInfo::scalar("Handle", "ptr"));
        }

        // non-const struct* -> out_struct (resolution to a concrete record
        // — or fallback to Handle for opaque types — happens in
        // crate::parse::function).
        if pointee_canonical.kind == CXType_Record && !const_pointee {
            let record = record_name(pointee_canonical)?;
            return Ok(ArgInfo {
                port: record.clone(),
                descriptor: "out_struct".into(),
                consumes: false,
                is_out_struct: true,
                is_in_struct: false,
                record_name: Some(record),
                is_size_pair: false,
            });
        }

        // const struct T* with no trailing length -> in_struct: the caller
        // supplies a Handl record and the marshaller builds the C struct
        // (e.g. clock_settime's `const struct timespec *`). The const+len
        // case (sockaddr* + socklen_t) was already taken by in_buf above.
        if const_pointee && pointee_canonical.kind == CXType_Record {
            let record = record_name(pointee_canonical)?;
            return Ok(ArgInfo {
                port: record.clone(),
                descriptor: "in_struct".into(),
                consumes: true,
                is_out_struct: false,
                is_in_struct: true,
                record_name: Some(record),
                is_size_pair: false,
            });
        }

        return Err(format!("unsupported pointer type: {}", type_spelling(ty)));
    }

    // `ty`, not `canonical` — the typedef spelling carries ctypes aliases
    // (`socklen_t` → `Socklen`) that canonicalization would erase.
    let (port, descriptor) = map_arg_type(ty, pointer_width)?;
    Ok(ArgInfo {
        port,
        descriptor,
        consumes: true,
        is_out_struct: false,
        is_in_struct: false,
        record_name: None,
        is_size_pair: false,
    })
}
