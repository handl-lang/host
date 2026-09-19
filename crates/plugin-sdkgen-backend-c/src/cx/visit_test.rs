use crate::cx::{children_of_kind, cursor_name, first_child};
use crate::testutil::parse_snippet;
use clang_sys::*;

#[test]
fn children_of_kind_collects_matching_only() {
    let sn = parse_snippet("struct p { int x; long y; };\n");
    let rec = sn.record_cursor("p");
    let fields = children_of_kind(rec, CXCursor_FieldDecl);
    assert_eq!(fields.len(), 2);
    assert_eq!(cursor_name(fields[0]), "x");
    assert_eq!(cursor_name(fields[1]), "y");
    // Non-matching kinds yield nothing.
    assert!(children_of_kind(rec, CXCursor_FunctionDecl).is_empty());
}

#[test]
fn first_child_gets_initializer_expr() {
    let sn = parse_snippet("enum { A = 1, B = 2 };\n");
    let e = sn.find_decl(CXCursor_EnumDecl, "").expect("anonymous enum");
    let first_const = first_child(e).expect("first constant");
    assert_eq!(cursor_name(first_const), "A");
    assert!(first_child(first_const).is_some());
}
