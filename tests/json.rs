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
fn json_module_exposes_functions() {
    let module = module("json").unwrap();
    assert_eq!(module.name(), "json");

    let mut ctx = EvalContext::new();
    let parse = module.function("parse").expect("parse should exist");
    let result = parse.call(&[json!("42")], &mut ctx).unwrap();
    assert_eq!(result, json!(42));

    let stringify = module
        .function("stringify")
        .expect("stringify should exist");
    let result = stringify.call(&[json!({"a": 1})], &mut ctx).unwrap();
    assert_eq!(result, json!("{\"a\":1}"));
}

#[test]
fn json_module_legacy_name_function() {
    let module = module("json").unwrap();
    let mut ctx = EvalContext::new();
    let name_func = module.function("json").expect("json should exist");
    let result = name_func.call(&[], &mut ctx).unwrap();
    assert_eq!(result, json!({"type": "serializer", "name": "json"}));
}

#[test]
fn plugin_id_and_name() {
    let plugin = library("json").unwrap();
    assert_eq!(plugin.id(), "lib.json");
    assert_eq!(plugin.name(), "lib-json");
}

#[test]
fn handl_plugin_id_and_name() {
    let plugin = library("json").unwrap();
    assert_eq!(plugin.id(), "lib.json");
    assert_eq!(plugin.name(), "lib-json");
}

#[test]
fn json_module_works_in_module_registry() {
    let module = module("json").unwrap();
    let registry = ModuleRegistry::new().with(module);
    let lookup: &dyn ModuleLookup = &registry;

    let func = lookup
        .lookup("json.parse")
        .expect("json.parse should resolve");
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!("\"hello\"")], &mut ctx).unwrap();
    assert_eq!(result, json!("hello"));
}

// -----------------------------------------------------------------------
// ADR-0077 Phase 5: capability declaration tests
// -----------------------------------------------------------------------

#[test]
fn json_plugin_exposes_declared_capabilities() {
    let plugin = library("json").unwrap();
    let caps = plugin.capabilities();
    assert_eq!(caps.len(), 2);
}
