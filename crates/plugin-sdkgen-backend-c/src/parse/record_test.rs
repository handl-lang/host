use crate::parse::{ParseCtx, parse_record};
use crate::testutil::parse_snippet;
use std::cell::RefCell;
use std::collections::HashMap;

fn ctx(sn: &crate::testutil::Snippet) -> ParseCtx {
    ParseCtx {
        pointer_width: sn.pointer_width(),
        records: RefCell::new(HashMap::new()),
    }
}

#[test]
fn record_fields_offsets_and_layout() {
    let sn = parse_snippet("/** Docs. */\nstruct p { int x; long long y; char pad; };\n");
    let rec = parse_record(sn.record_cursor("p"), &ctx(&sn)).unwrap();
    assert_eq!(rec.name, "p");
    assert_eq!(rec.fields.len(), 3);
    assert_eq!(rec.fields[0].name, "x");
    assert_eq!(rec.fields[0].descriptor, "i32");
    assert_eq!(rec.fields[0].offset, 0);
    assert_eq!(rec.fields[1].name, "y");
    assert_eq!(rec.fields[1].descriptor, "i64");
    assert_eq!(rec.fields[1].offset, 8); // aligned after int
    assert_eq!(rec.fields[2].descriptor, "i8");
    assert!(rec.size > 0 && rec.align > 0);
    assert!(rec.doc.unwrap().contains("Docs"));
}

#[test]
fn bitfield_record_errors() {
    let sn = parse_snippet("struct b { unsigned int a : 1; unsigned int r : 7; };\n");
    let err = parse_record(sn.record_cursor("b"), &ctx(&sn)).unwrap_err();
    assert!(err.contains("bitfield"), "{err}");
}

#[test]
fn array_field_record_errors() {
    let sn = parse_snippet("struct a { int x; char buf[8]; };\n");
    let err = parse_record(sn.record_cursor("a"), &ctx(&sn)).unwrap_err();
    assert!(err.contains("unsupported field type"), "{err}");
}

#[test]
fn anonymous_struct_errors() {
    // A truly anonymous tag (`struct {..} glob;`) has no usable name.
    let sn = parse_snippet("struct { int x; } glob;\n");
    let c = sn
        .find_decl(clang_sys::CXCursor_StructDecl, "")
        .expect("anon struct");
    assert!(
        parse_record(c, &ctx(&sn))
            .unwrap_err()
            .contains("anonymous")
    );
}

#[test]
fn typedef_anonymous_struct_uses_typedef_name() {
    // `typedef struct {..} anon_t` — libclang names the decl "anon_t",
    // so it is a normal named record.
    let sn = parse_snippet("typedef struct { int x; } anon_t;\n");
    let rec = parse_record(sn.record_cursor("anon_t"), &ctx(&sn)).unwrap();
    assert_eq!(rec.name, "anon_t");
    assert_eq!(rec.fields.len(), 1);
}
