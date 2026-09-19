use crate::parse::{ParseCtx, parse_typedef};
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
fn typedef_maps_through_underlying() {
    let sn = parse_snippet("typedef unsigned int myuint;\ntypedef long mylong;\n");
    let td = parse_typedef(sn.typedef_cursor("myuint"), &ctx(&sn)).unwrap();
    assert_eq!(td.name, "myuint");
    assert_eq!(td.port, "Nat");
    assert_eq!(td.descriptor, "i32");
    let td = parse_typedef(sn.typedef_cursor("mylong"), &ctx(&sn)).unwrap();
    assert_eq!((td.port.as_str(), td.descriptor.as_str()), ("Int", "i64"));
}

#[test]
fn typedef_of_size_t_uses_pointer_width() {
    let sn = parse_snippet("typedef unsigned long size_t;\n");
    let td = parse_typedef(sn.typedef_cursor("size_t"), &ctx(&sn)).unwrap();
    assert_eq!(td.port, "Nat");
    assert_eq!(td.descriptor, "i64"); // 64-bit host
}
