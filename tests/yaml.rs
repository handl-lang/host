use handl_platform::{EvalContext, Function, Module, ModuleLookup, Plugin};
use handl_plugin_lib_serialize::{Decoder, Encoder};
use handl_plugin_lib_stdlib::ops::yaml::*;
use handl_plugin_lib_stdlib::{library, module};
use handl_plugin_module::ModuleRegistry;
use serde_json::json;

#[test]
fn round_trip_document() {
    let input = r#"
hello: world
list:
  - 1
  - 2
  - 3
nested:
  answer: 42
"#;
    let value = Yaml.decode(input).unwrap();
    let output = Yaml.encode(&value).unwrap();
    let round = Yaml.decode(&output).unwrap();
    assert_eq!(value, round);
}

#[test]
fn rejects_invalid_yaml() {
    assert!(Yaml.decode("  [").is_err());
}

#[test]
fn parse_works() {
    let func = YamlParse;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!("hello: world\n")], &mut ctx)
        .unwrap_or_else(|e| panic!("parse failed: {e}"));
    assert_eq!(result, json!({"hello": "world"}));
}

#[test]
fn stringify_works() {
    let func = YamlStringify;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!({"hello": "world"})], &mut ctx)
        .unwrap_or_else(|e| panic!("stringify failed: {e}"));
    assert!(result.as_str().unwrap().contains("hello: world"));
}

#[test]
fn parse_stringify_roundtrip() {
    let parse = YamlParse;
    let stringify = YamlStringify;
    let mut ctx = EvalContext::new();
    let original = json!("hello: world\nlist:\n  - 1\n  - 2\n");
    let value = parse.call(&[original.clone()], &mut ctx).unwrap();
    let text = stringify.call(&[value.clone()], &mut ctx).unwrap();
    let round = parse.call(&[text], &mut ctx).unwrap();
    assert_eq!(round, value);
}

#[test]
fn parse_rejects_non_string() {
    let func = YamlParse;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!(42)], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("expects a string"));
}

#[test]
fn parse_rejects_invalid_yaml() {
    let func = YamlParse;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!("  [")], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("yaml parse error"));
}

#[test]
fn stringify_rejects_wrong_arity() {
    let func = YamlStringify;
    let mut ctx = EvalContext::new();
    let err = func.call(&[], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("expected 1 args"));
}

#[test]
fn op_registry_registers_both_ops() {
    let registry = yaml_op_registry();
    assert!(registry.contains("yaml.parse"));
    assert!(registry.contains("yaml.stringify"));
    assert_eq!(registry.len(), 3);
}

#[test]
fn yaml_ops_via_op_registry() {
    let registry = yaml_op_registry();
    let mut ctx = EvalContext::new();
    let parse = registry.get("yaml.parse").expect("parse should exist");
    let result = parse.call(&[json!("hello: world\n")], &mut ctx).unwrap();
    assert_eq!(result, json!({"hello": "world"}));

    let stringify = registry
        .get("yaml.stringify")
        .expect("stringify should exist");
    let result = stringify.call(&[json!({"a": 1})], &mut ctx).unwrap();
    assert!(result.as_str().unwrap().contains("a: 1"));
}

#[test]
fn yaml_legacy_name_op() {
    let registry = yaml_op_registry();
    let mut ctx = EvalContext::new();
    let name_func = registry.get("yaml.yaml").expect("yaml op should exist");
    let result = name_func.call(&[], &mut ctx).unwrap();
    assert_eq!(result, json!({"type": "serializer", "name": "yaml"}));
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
