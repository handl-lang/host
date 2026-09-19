use crate::cx::{cursor_name, doc_comment, record_name, type_spelling};
use crate::testutil::parse_snippet;
use clang_sys::*;

#[test]
fn cursor_and_type_names() {
    let sn = parse_snippet("struct point { int x; };\nint add(int a, int b);\n");
    assert_eq!(cursor_name(sn.fn_cursor("add")), "add");
    assert_eq!(cursor_name(sn.record_cursor("point")), "point");
    assert_eq!(type_spelling(sn.fn_arg_type("add", 0)), "int");

    let ret = sn.fn_ret_type("add");
    assert_eq!(ret.kind, CXType_Int);
}

#[test]
fn doc_comment_extracts_doc() {
    let sn = parse_snippet("/** Adds things. */\nint add(int a, int b);\nint plain(void);\n");
    assert_eq!(doc_comment(sn.fn_cursor("add")).unwrap(), "Adds things.");
    assert!(doc_comment(sn.fn_cursor("plain")).is_none());
}

#[test]
fn doc_comment_translates_doxygen() {
    let sn = parse_snippet(
        "/**\n * @brief Open a socket.\n * @param domain  protocol family\n * @param[out] type socket type\n * @return new fd\n */\nint open_sock(int domain, int type);\n",
    );
    assert_eq!(
        doc_comment(sn.fn_cursor("open_sock")).unwrap(),
        "Open a socket.\ndomain: protocol family\ntype: socket type\nReturns: new fd"
    );
}

#[test]
fn doc_comment_strips_slashes() {
    let sn = parse_snippet("/// Triple-slash doc.\nint f(void);\n");
    assert_eq!(doc_comment(sn.fn_cursor("f")).unwrap(), "Triple-slash doc.");
}

#[test]
fn record_name_resolves_struct_pointee() {
    let sn = parse_snippet("struct point { int x; };\nint mv(struct point *p);\n");
    let arg = sn.fn_arg_type("mv", 0);
    let pointee = unsafe { clang_getPointeeType(clang_getCanonicalType(arg)) };
    assert_eq!(record_name(pointee).unwrap(), "point");
}

#[test]
fn record_name_rejects_anonymous() {
    let sn = parse_snippet("int mv(struct { int x; } *p);\n");
    let arg = sn.fn_arg_type("mv", 0);
    let pointee = unsafe { clang_getPointeeType(clang_getCanonicalType(arg)) };
    assert!(record_name(pointee).unwrap_err().contains("unnamed"));
}
