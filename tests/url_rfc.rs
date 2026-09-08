use handl_platform::{EvalContext, Function, Plugin};
use handl_plugin_lib_parse::DynParser;
use handl_plugin_lib_stdlib::ops::url_rfc::*;
use handl_plugin_lib_stdlib::{library, module, module_ref};
use serde_json::json;
use std::sync::Arc;

#[test]
fn parses_http_uri() {
    let u = handl_plugin_lib_parse::parse("https://example.com:8080/path?a=1#frag", Rfc).unwrap();
    assert_eq!(u.scheme(), "https");
    assert_eq!(u.authority(), Some("example.com:8080"));
    assert_eq!(u.path(), "/path");
    assert_eq!(u.query(), Some("a=1"));
    assert_eq!(u.fragment(), Some("frag"));
}

#[test]
fn rejects_non_ascii() {
    assert!(handl_plugin_lib_parse::parse("https://example.com/🦀", Rfc).is_err());
}

#[test]
fn rejects_invalid_percent_encoding() {
    assert!(handl_plugin_lib_parse::parse("https://example.com/%ZZ", Rfc).is_err());
}

#[test]
fn does_not_normalize_path() {
    let u = handl_plugin_lib_parse::parse("https://example.com/../a/../b", Rfc).unwrap();
    assert_eq!(u.path(), "/../a/../b");
}

#[test]
fn parses_to_value() {
    let v = DynParser::parse(&Rfc, "https://example.com:8080/path?a=1#frag").unwrap();
    assert_eq!(v["scheme"], "https");
    assert_eq!(v["authority"], "example.com:8080");
    assert_eq!(v["path"], "/path");
}

#[test]
fn url_rfc_object_is_parser_token() {
    let mut ctx = EvalContext::new();
    let obj = UrlRfcName.call(&[], &mut ctx).unwrap();
    assert_eq!(obj["type"], "parser");
    assert_eq!(obj["name"], "url_rfc");
    assert!(obj.get("parse").is_none());
}

#[test]
fn op_registry_registers_both_ops() {
    let registry = url_rfc_op_registry();
    assert!(registry.contains("url_rfc.parse"));
    assert!(registry.contains("url_rfc.url_rfc"));
    assert_eq!(registry.len(), 2);
}

#[test]
fn url_rfc_parse_op_works() {
    let registry = Arc::new(url_rfc_op_registry());
    let op_func = handl_plugin_lib_handl::OpFunction::new("url_rfc.parse", registry);
    let mut ctx = EvalContext::new();
    let result = op_func
        .call(&[json!("https://example.com:8080/path?a=1#frag")], &mut ctx)
        .unwrap();
    assert_eq!(result["scheme"], "https");
    assert_eq!(result["authority"], "example.com:8080");
    assert_eq!(result["path"], "/path");
}

#[test]
fn plugin_id_and_name() {
    let plugin = library("url_rfc").unwrap();
    assert_eq!(plugin.id(), "lib.url-rfc");
    assert_eq!(plugin.name(), "lib-url-rfc");
}
