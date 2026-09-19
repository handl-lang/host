use crate::parse::{ParseCtx, parse_function, parse_record};
use crate::testutil::parse_snippet;
use std::cell::RefCell;
use std::collections::HashMap;

const PRELUDE: &str = "typedef unsigned long size_t;\ntypedef unsigned int socklen_t;\n";

fn ctx(sn: &crate::testutil::Snippet) -> ParseCtx {
    ParseCtx {
        pointer_width: sn.pointer_width(),
        records: RefCell::new(HashMap::new()),
    }
}

fn parse(src: &str, fname: &str) -> Result<handl_plugin_sdkgen::Function, String> {
    let sn = parse_snippet(src);
    parse_function(sn.fn_cursor(fname), &ctx(&sn))
}

#[test]
fn scalar_args_and_pascal_port_name() {
    let f = parse(
        &format!("{PRELUDE}int sock_send_v2(int fd, int flags);\n"),
        "sock_send_v2",
    )
    .unwrap();
    assert_eq!(f.port_name, "SockSendV2");
    assert_eq!(f.port_args.len(), 2);
    assert_eq!(f.arg_descriptors, &["i32", "i32"]);
    assert_eq!(f.return_port, "Int");
}

#[test]
fn ptr_len_pair_collapses() {
    let f = parse(
        &format!("{PRELUDE}int send_(int fd, const void *b, size_t n, int fl);\n"),
        "send_",
    )
    .unwrap();
    assert_eq!(f.port_args.len(), 3);
    assert_eq!(f.port_args[1].name, "b");
    assert_eq!(f.port_args[1].port, "Bytes");
    assert_eq!(f.arg_descriptors, &["i32", "in_buf", "i32"]);
}

#[test]
fn void_param_list_is_no_args() {
    let f = parse(&format!("{PRELUDE}int now(void);\n"), "now").unwrap();
    assert!(f.port_args.is_empty());
    assert!(f.arg_descriptors.is_empty());
}

#[test]
fn out_struct_resolves_to_descriptor() {
    let src = "struct p { int x; int y; };\nint mv(struct p *o, int dx);\n";
    let sn = parse_snippet(src);
    let c = ctx(&sn);
    // The record must be parsed into ctx first — mirrors backend order.
    let rec = parse_record(sn.record_cursor("p"), &c).unwrap();
    c.records.borrow_mut().insert(rec.name.clone(), rec);
    let f = parse_function(sn.fn_cursor("mv"), &c).unwrap();
    assert_eq!(f.out_struct.as_deref(), Some("p"));
    // out_struct returns keep `ret` — the port declares `{Port}Result`
    // `{ret, buf: p}` rather than unwrapping and dropping the status.
    assert_eq!(f.return_port, "MvResult");
    // Field specs carry real libclang layout (`name:kind@off`).
    assert_eq!(f.arg_descriptors, &["out_struct:{x:i32@0,y:i32@4}", "i32"]);
    assert_eq!(f.port_args.len(), 1); // out-struct consumes no handl arg
}

#[test]
fn opaque_struct_ptr_falls_back_to_handle() {
    let f = parse(
        &format!("{PRELUDE}struct ctx;\nint ping(struct ctx *c);\n"),
        "ping",
    )
    .unwrap();
    assert_eq!(f.port_args.len(), 1);
    assert_eq!(f.port_args[0].port, "Handle");
    assert_eq!(f.arg_descriptors, &["ptr"]);
    assert!(f.out_struct.is_none());
}

#[test]
fn multiple_out_structs_error() {
    let src = "struct p { int x; };\nint two(struct p *a, struct p *b);\n";
    let sn = parse_snippet(src);
    let c = ctx(&sn);
    let rec = parse_record(sn.record_cursor("p"), &c).unwrap();
    c.records.borrow_mut().insert(rec.name.clone(), rec);
    let err = parse_function(sn.fn_cursor("two"), &c).unwrap_err();
    assert!(err.contains("multiple out-struct"), "{err}");
}

#[test]
fn variadic_flag_and_int_not_len() {
    let f = parse(
        &format!("{PRELUDE}int v(const char *fmt, int n, ...);\n"),
        "v",
    )
    .unwrap();
    assert!(f.variadic);
    // int does not count as length for a variadic fn: fmt stays cstr.
    assert_eq!(f.arg_descriptors, &["cstr", "i32"]);
    assert_eq!(f.port_args.len(), 2);
}

#[test]
fn char_ptr_return_is_cstr() {
    let f = parse(&format!("{PRELUDE}const char *msg(int fd);\n"), "msg").unwrap();
    assert_eq!(f.return_port, "String");
    assert_eq!(f.ret_descriptor, "cstr");
}
