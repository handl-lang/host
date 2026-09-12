//! Tests for the `result` table module / package (folded from
//! `plugin-lib-result`).
//!
//! The `result` module's `tag` dependency is not in the package table
//! (`tag` stays in `plugin-lib-tag`), so the tests build the module via
//! [`module_with`] and a resolver that maps `"tag"` → `TagModule`.

use std::sync::Arc;

use handl_platform::{EvalContext, Module, Plugin};
use handl_plugin_lib_handl::ModuleResolver;
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use serde_json::json;

/// Resolver: the package table, plus `"tag"` → `TagModule`.
struct TagResolver;

impl ModuleResolver for TagResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        if name == "tag" {
            return Some(Arc::new(TagModule));
        }
        stdlib::DEFAULT_RESOLVER.resolve(name)
    }
}

/// Helper: call a result function by name with the given args.
fn call(name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let module = stdlib::module_with("result", Arc::new(TagResolver)).expect("result module should exist");
    let func = module
        .function(name)
        .unwrap_or_else(|| panic!("result.{name} should exist"));
    func.call(args, &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("result.{name} failed: {e}"))
}

#[test]
fn ok_wraps_value() {
    let result = call("ok", &[json!(42.0)]);
    assert_eq!(result, json!({"__tag__": "ok", "value": 42.0}));
}

#[test]
fn err_wraps_error() {
    let result = call("err", &[json!("nope")]);
    assert_eq!(result, json!({"__tag__": "err", "value": "nope"}));
}

#[test]
fn ok_and_err_have_different_tags() {
    let ok_val = call("ok", &[json!(42.0)]);
    let err_val = call("err", &[json!(42.0)]);
    assert_ne!(ok_val, err_val);
    assert_eq!(ok_val.get("__tag__"), Some(&json!("ok")));
    assert_eq!(err_val.get("__tag__"), Some(&json!("err")));
}

#[test]
fn ok_wraps_string() {
    let result = call("ok", &[json!("hello")]);
    assert_eq!(result, json!({"__tag__": "ok", "value": "hello"}));
}

#[test]
fn err_wraps_number() {
    let result = call("err", &[json!(404.0)]);
    assert_eq!(result, json!({"__tag__": "err", "value": 404.0}));
}

#[test]
fn module_name_is_result() {
    let module = stdlib::module("result").expect("result module should exist");
    assert_eq!(module.name(), "result");
}

#[test]
fn module_exposes_ok_and_err() {
    let module = stdlib::module("result").expect("result module should exist");
    assert!(module.function("ok").is_some());
    assert!(module.function("err").is_some());
    assert!(module.function("is_ok").is_none());
    assert!(module.function("is_err").is_none());
}

#[test]
fn plugin_id_and_name_are_preserved() {
    let plugin = stdlib::library("result").expect("result package should exist");
    assert_eq!(plugin.id(), "lib.result");
    assert_eq!(plugin.name(), "lib-result");
}
