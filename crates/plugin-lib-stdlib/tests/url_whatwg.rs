use handl_platform::{EvalContext, Function, Plugin};
use handl_lib_parse::DynParser;
use handl_lib_url_whatwg::ops::*;
use handl_plugin_lib_stdlib::library;
use serde_json::json;
use std::sync::Arc;

#[test]
fn parses_http_url() {
    let u =
        handl_lib_parse::parse("https://example.com:8080/path?a=1#frag", Whatwg).unwrap();
    assert_eq!(u.scheme(), "https");
    assert_eq!(u.host(), Some("example.com"));
    assert_eq!(u.port(), Some(8080));
    assert_eq!(u.path(), "/path");
    assert_eq!(u.query(), Some("a=1"));
    assert_eq!(u.fragment(), Some("frag"));
}

#[test]
fn whatwg_normalizes_invalid() {
    let u = handl_lib_parse::parse("HTtpS://EXAMPLE.com/../a/../b", Whatwg).unwrap();
    assert_eq!(u.scheme(), "https");
    assert_eq!(u.host(), Some("example.com"));
    assert_eq!(u.path(), "/b");
}

#[test]
fn rejects_garbage() {
    assert!(handl_lib_parse::parse("not a url", Whatwg).is_err());
}

#[test]
fn parses_to_value() {
    let v = DynParser::parse(&Whatwg, "https://example.com:8080/path?a=1#frag").unwrap();
    assert_eq!(v["scheme"], "https");
    assert_eq!(v["host"], "example.com");
    assert_eq!(v["path"], "/path");
}

#[test]
fn url_whatwg_object_is_parser_token() {
    let mut ctx = EvalContext::new();
    let obj = UrlWhatwgName.call(&[], &mut ctx).unwrap();
    assert_eq!(obj["type"], "parser");
    assert_eq!(obj["name"], "url_whatwg");
    assert!(obj.get("parse").is_none());
}

#[test]
fn op_registry_registers_both_ops() {
    let registry = url_whatwg_op_registry();
    assert!(registry.contains("url_whatwg.parse"));
    assert!(registry.contains("url_whatwg.url_whatwg"));
    assert_eq!(registry.len(), 2);
}

#[test]
fn url_whatwg_parse_op_works() {
    let registry = Arc::new(url_whatwg_op_registry());
    let op_func = handl_plugin_lib_handl::OpFunction::new("url_whatwg.parse", registry);
    let mut ctx = EvalContext::new();
    let result = op_func
        .call(&[json!("https://example.com:8080/path?a=1#frag")], &mut ctx)
        .unwrap();
    assert_eq!(result["scheme"], "https");
    assert_eq!(result["host"], "example.com");
    assert_eq!(result["path"], "/path");
}

#[test]
fn plugin_id_and_name() {
    let plugin = library("url_whatwg").unwrap();
    assert_eq!(plugin.id(), "lib.url-whatwg");
    assert_eq!(plugin.name(), "lib-url-whatwg");
}
