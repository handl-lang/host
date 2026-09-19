//! `CXType` kind predicates.

use clang_sys::*;

use crate::cx::type_spelling;

pub(crate) fn is_void(ty: CXType) -> bool {
    ty.kind == CXType_Void
}

pub(crate) fn is_char(ty: CXType) -> bool {
    matches!(
        ty.kind,
        CXType_Char_S | CXType_UChar | CXType_Char_U | CXType_SChar
    )
}

pub(crate) fn is_size_t(ty: CXType) -> bool {
    type_spelling(ty) == "size_t"
}

/// Whether an argument reads as the length half of a `(ptr, len)` pair —
/// by type (`size_t`, `ssize_t`, `socklen_t`, ...) or by parameter name
/// (`len`, `*len`, `*size`, `*count`, `n`, `nbytes`, `capacity`, `num`).
/// Bare `int`/`flags`-style arguments deliberately no longer count — a
/// wrong collapse silently binds e.g. `flags` as a buffer length.
pub(crate) fn is_len_like(ty: CXType, name: &str) -> bool {
    // Only integer (or enum) arguments can be lengths — a `float len`
    // must not collapse a pair.
    let canonical = unsafe { clang_getCanonicalType(ty) };
    let is_int = matches!(
        canonical.kind,
        CXType_SChar
            | CXType_UChar
            | CXType_Char_S
            | CXType_Char_U
            | CXType_Short
            | CXType_UShort
            | CXType_Int
            | CXType_UInt
            | CXType_Long
            | CXType_ULong
            | CXType_LongLong
            | CXType_ULongLong
            | CXType_Enum
    );
    if !is_int {
        return false;
    }
    let sp = type_spelling(ty);
    if matches!(
        sp.as_str(),
        "size_t" | "ssize_t" | "socklen_t" | "off_t" | "long" | "unsigned long"
    ) {
        return true;
    }
    let n = name.trim_start_matches('_'); // glibc uses __len, __n, __count
    n == "n"
        || n == "len"
        || n == "nbytes"
        || n == "capacity"
        || n == "num"
        || n.ends_with("len")
        || n.ends_with("size")
        || n.ends_with("count")
}
