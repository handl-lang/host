use handl_lib_serialize::{Decoder, Encoder};
use handl_platform::{EvalContext, Function, ModuleLookup, Plugin};
use handl_plugin_lib_stdlib::ops::toml::*;
use handl_plugin_lib_stdlib::{library, module};
use handl_plugin_module::ModuleRegistry;
use serde_json::json;

#[test]
fn round_trip_document() {
    let input = r#"
title = "example"
[owner]
name = "luis"
nums = [1, 2, 3]
active = true
"#;
    let value = Toml.decode(input).unwrap();
    let output = Toml.encode(&value).unwrap();
    let round = Toml.decode(&output).unwrap();
    assert_eq!(value, round);
}

#[test]
fn rejects_invalid_toml() {
    assert!(Toml.decode("not toml").is_err());
}

#[test]
fn parse_works() {
    let func = TomlParse;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!("title = \"example\"")], &mut ctx)
        .unwrap_or_else(|e| panic!("parse failed: {e}"));
    assert_eq!(result, json!({"title": "example"}));
}

#[test]
fn stringify_works() {
    let func = TomlStringify;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!({"title": "example"})], &mut ctx)
        .unwrap_or_else(|e| panic!("stringify failed: {e}"));
    assert!(result.as_str().unwrap().contains("title = \"example\""));
}

#[test]
fn parse_stringify_roundtrip() {
    let parse = TomlParse;
    let stringify = TomlStringify;
    let mut ctx = EvalContext::new();
    let original = json!("title = \"example\"\nnums = [1, 2, 3]\n");
    let value = parse.call(&[original.clone()], &mut ctx).unwrap();
    let text = stringify.call(&[value.clone()], &mut ctx).unwrap();
    let round = parse.call(&[text], &mut ctx).unwrap();
    assert_eq!(round, value);
}

#[test]
fn parse_rejects_non_string() {
    let func = TomlParse;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!(42)], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("expects a string"));
}

#[test]
fn parse_rejects_invalid_toml() {
    let func = TomlParse;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!("not toml = ")], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("toml parse error"));
}

#[test]
fn stringify_rejects_wrong_arity() {
    let func = TomlStringify;
    let mut ctx = EvalContext::new();
    let err = func.call(&[], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("expected 1 args"));
}

#[test]
fn op_registry_registers_both_ops() {
    let registry = toml_op_registry();
    assert!(registry.contains("toml.parse"));
    assert!(registry.contains("toml.stringify"));
    assert_eq!(registry.len(), 3);
}

#[test]
fn toml_ops_via_op_registry() {
    let registry = toml_op_registry();
    let mut ctx = EvalContext::new();
    let parse = registry.get("toml.parse").expect("parse should exist");
    let result = parse.call(&[json!("title = \"x\"")], &mut ctx).unwrap();
    assert_eq!(result, json!({"title": "x"}));

    let stringify = registry
        .get("toml.stringify")
        .expect("stringify should exist");
    let result = stringify.call(&[json!({"a": 1})], &mut ctx).unwrap();
    assert!(result.as_str().unwrap().contains("a = 1"));
}

#[test]
fn toml_legacy_name_op() {
    let registry = toml_op_registry();
    let mut ctx = EvalContext::new();
    let name_func = registry.get("toml.toml").expect("toml op should exist");
    let result = name_func.call(&[], &mut ctx).unwrap();
    assert_eq!(result, json!({"type": "serializer", "name": "toml"}));
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
    // Only the json backend is registered; the format arg must match.
    let _ = func;
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
