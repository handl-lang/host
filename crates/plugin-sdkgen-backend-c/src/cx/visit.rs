//! Generic `clang_visitChildren` helpers.
//!
//! One `kind_collector` callback driven by a target `CXCursorKind` in the
//! client data replaces the former per-kind collectors
//! (`field_collector`, `enum_collector`, `enum_decl_collector`).

use clang_sys::*;
use std::ffi::c_void;

/// State for [`kind_collector`]: the kind to match plus the sink.
struct KindCollect {
    kind: CXCursorKind,
    cursors: Vec<CXCursor>,
}

extern "C" fn kind_collector(
    cursor: CXCursor,
    _parent: CXCursor,
    data: *mut c_void,
) -> CXChildVisitResult {
    let st = unsafe { &mut *(data as *mut KindCollect) };
    if cursor.kind == st.kind {
        st.cursors.push(cursor);
    }
    CXChildVisit_Continue
}

/// Collect direct children of `cursor` whose kind is `kind`
/// (`CXCursor_FieldDecl`, `CXCursor_EnumConstantDecl`, ...).
pub(crate) fn children_of_kind(cursor: CXCursor, kind: CXCursorKind) -> Vec<CXCursor> {
    let mut st = KindCollect {
        kind,
        cursors: Vec::new(),
    };
    unsafe { clang_visitChildren(cursor, kind_collector, &mut st as *mut _ as *mut c_void) };
    st.cursors
}

extern "C" fn first_child_visitor(
    cursor: CXCursor,
    _parent: CXCursor,
    data: *mut c_void,
) -> CXChildVisitResult {
    let v = unsafe { &mut *(data as *mut Vec<CXCursor>) };
    v.push(cursor);
    CXChildVisit_Break
}

/// The first child of `cursor`, if any.
pub(crate) fn first_child(cursor: CXCursor) -> Option<CXCursor> {
    let mut v: Vec<CXCursor> = Vec::new();
    unsafe { clang_visitChildren(cursor, first_child_visitor, &mut v as *mut _ as *mut c_void) };
    v.into_iter().next()
}
