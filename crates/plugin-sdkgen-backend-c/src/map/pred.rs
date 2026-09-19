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

/// Whether a type's canonical kind is an integer or enum — used to detect
/// the length argument of a `(ptr, len)` pair (`size_t`, `socklen_t`, ...).
pub(crate) fn is_integer_kind(ty: CXType) -> bool {
    let canonical = unsafe { clang_getCanonicalType(ty) };
    matches!(
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
    )
}
