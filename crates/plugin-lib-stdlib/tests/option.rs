//! Tests for the `option` table module / package (folded from
//! `plugin-lib-result`'s sibling).
//!
//! The `option` module's `tag` dependency is not in the package table
//! (`tag` stays in `plugin-lib-tag`), so the tests build the module via
//! [`module_with`] and a resolver that maps `"tag"` → `TagModule`.
//!
//! The old native `option` op registry (`option.map`, `option.filter`,
//! `option.unwrap_or`, …) was deleted in `fa89d551`; the surviving surface
//! is the `Option`/`some`/`none` contract in `option/index.hnl`, exercised
//! end-to-end by `handl-packages/option/option.test.hnl`.

use std::sync::Arc;

use handl_platform::{EvalContext, Module};
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

/// Helper: call an option function by name with the given args.
fn call(name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let module =
        stdlib::module_with("option", Arc::new(TagResolver)).expect("option module should exist");
    let func = module
        .function(name)
        .unwrap_or_else(|| panic!("option.{name} should exist"));
    func.call(args, &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("option.{name} failed: {e}"))
}

#[test]
fn option_module_exposes_constructors() {
    let module =
        stdlib::module_with("option", Arc::new(TagResolver)).expect("option module should exist");
    assert!(module.function("some").is_some());
    assert!(module.function("none").is_some());
}

#[test]
fn some_wraps_value() {
    let result = call("some", &[json!(42.0)]);
    assert_eq!(result, json!({"__tag__": "Some", "value": 42.0}));
}

#[test]
fn some_wraps_string() {
    let result = call("some", &[json!("hi")]);
    assert_eq!(result, json!({"__tag__": "Some", "value": "hi"}));
}

#[test]
fn none_constructs_empty_tag() {
    let result = call("none", &[]);
    assert_eq!(result, json!({"__tag__": "None"}));
}
