use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup, Plugin};
use handl_plugin_lib_serialize::{Decoder, Encoder, Serializer};
use handl_plugin_lib_stdlib::ops::json::*;
use handl_plugin_lib_stdlib::{library, module, module_ref};
use handl_plugin_module::ModuleRegistry;
use serde_json::{Value, json};
use std::sync::Arc;

#[test]
fn round_trip_object() {
    let input = r#"{"hello":"world","list":[1,2,3]}"#;
    let value = Json.decode(input).unwrap();
    let output = Json.encode(&value).unwrap();
    let round = Json.decode(&output).unwrap();
    assert_eq!(value, round);
}

#[test]
fn round_trip_primitives() {
    for input in ["\"hello\"", "42", "true", "null", "[1,2]"] {
        let value = Json.decode(input).unwrap();
        let output = Json.encode(&value).unwrap();
        let round = Json.decode(&output).unwrap();
        assert_eq!(value, round);
    }
}

#[test]
fn rejects_invalid_json() {
    assert!(Json.decode("not json").is_err());
}

#[test]
fn parse_works() {
    let func = JsonParse;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!("{\"hello\":\"world\"}")], &mut ctx)
        .unwrap_or_else(|e| panic!("parse failed: {e}"));
    assert_eq!(result, json!({"hello": "world"}));
}

#[test]
fn stringify_works() {
    let func = JsonStringify;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!({"hello": "world"})], &mut ctx)
        .unwrap_or_else(|e| panic!("stringify failed: {e}"));
    assert_eq!(result, json!("{\"hello\":\"world\"}"));
}

#[test]
fn parse_stringify_roundtrip() {
    let parse = JsonParse;
    let stringify = JsonStringify;
    let mut ctx = EvalContext::new();
    let original = json!("{\"hello\":\"world\",\"list\":[1,2,3]}");
    let value = parse.call(&[original.clone()], &mut ctx).unwrap();
    let text = stringify.call(&[value.clone()], &mut ctx).unwrap();
    let round = parse.call(&[text], &mut ctx).unwrap();
    assert_eq!(round, value);
}

#[test]
fn parse_rejects_non_string() {
    let func = JsonParse;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!(42)], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("expects a string"));
}

#[test]
fn parse_rejects_invalid_json() {
    let func = JsonParse;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!("not json")], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("json parse error"));
}

#[test]
fn stringify_rejects_wrong_arity() {
    let func = JsonStringify;
    let mut ctx = EvalContext::new();
    let err = func.call(&[], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("expected 1 args"));
}

#[test]
fn op_registry_registers_both_ops() {
    let registry = json_op_registry();
    assert!(registry.contains("json.parse"));
    assert!(registry.contains("json.stringify"));
    assert_eq!(registry.len(), 3);
}

#[test]
fn json_ops_via_op_registry() {
    let registry = json_op_registry();
    let mut ctx = EvalContext::new();
    let parse = registry.get("json.parse").expect("parse should exist");
    let result = parse.call(&[json!("42")], &mut ctx).unwrap();
    assert_eq!(result, json!(42));

    let stringify = registry
        .get("json.stringify")
        .expect("stringify should exist");
    let result = stringify.call(&[json!({"a": 1})], &mut ctx).unwrap();
    assert_eq!(result, json!("{\"a\":1}"));
}

#[test]
fn json_legacy_name_op() {
    let registry = json_op_registry();
    let mut ctx = EvalContext::new();
    let name_func = registry
        .get("json.json")
        .expect("json op should exist");
    let result = name_func.call(&[], &mut ctx).unwrap();
    assert_eq!(result, json!({"type": "serializer", "name": "json"}));
}

#[test]
fn utils_plugin_id_and_name() {
    let plugin = library("utils").unwrap();
    assert_eq!(plugin.id(), "lib.utils");
    assert_eq!(plugin.name(), "lib-utils");
}

#[test]
fn serialize_module_works_in_module_registry() {
    let module = module("serialize").unwrap();
    let registry = ModuleRegistry::new().with(module);
    let lookup: &dyn ModuleLookup = &registry;

    assert!(lookup.lookup("serialize.serialize").is_some());
    let func = lookup
        .lookup("serialize.deserialize")
        .expect("serialize.deserialize should resolve");
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!("42"), json!("json")], &mut ctx).unwrap();
    assert_eq!(result, json!(42));
}

// -----------------------------------------------------------------------
// ADR-0077 Phase 5: capability declaration tests
// -----------------------------------------------------------------------

#[test]
fn utils_plugin_declares_serialize_capabilities() {
    let plugin = library("utils").unwrap();
    let caps = plugin.capabilities();
    assert!(
        caps.iter().any(|c| c.as_str() == "serialize.serialize"),
        "missing serialize.serialize cap"
    );
    assert!(
        caps.iter().any(|c| c.as_str() == "serialize.deserialize"),
        "missing serialize.deserialize cap"
    );
}
