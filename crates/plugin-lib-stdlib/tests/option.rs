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

/// Install the canonical default language once per test binary (ADR-0128
/// §"The 0.8 seam"): `HandlModule` bridge runtimes resolve it lazily.
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

/// Helper: call an option function by name with the given args.
fn call(name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    ensure_lang();
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

/// Phase-3 WS-F: `some`/`none` mint the canonical `__ctor__` object
/// shape (ADR-0152 §2 — the post-`__tag__` mint): `{"__ctor__": <sym>,
/// value: _, type: {__former__: <family sym>, indices: _}}`.
#[test]
fn some_wraps_value() {
    let result = call("some", &[json!(42.0)]);
    assert_eq!(result["value"], json!(42.0));
    assert_eq!(result["__ctor__"]["__sym__"]["name"], json!("Some"));
    assert_eq!(
        result["type"]["__former__"]["__sym__"]["name"],
        json!("Option")
    );
}

#[test]
fn some_wraps_string() {
    let result = call("some", &[json!("hi")]);
    assert_eq!(result["value"], json!("hi"));
    assert_eq!(result["__ctor__"]["__sym__"]["name"], json!("Some"));
}

#[test]
fn none_constructs_ctor_object() {
    let result = call("none", &[]);
    assert_eq!(result["__ctor__"]["__sym__"]["name"], json!("None"));
    assert_eq!(
        result["type"]["__former__"]["__sym__"]["name"],
        json!("Option")
    );
    assert!(result.get("value").is_none() || result["value"].is_null());
}

/// `Some`/`None` are also exported under their ctor names — `Some` the
/// constructor arrow, `None` the nullary ctor object (so `match v {
/// None: … }` discriminates by equality; ADR-0152 §2).
#[test]
fn option_module_exposes_ctor_names() {
    let module =
        stdlib::module_with("option", Arc::new(TagResolver)).expect("option module should exist");
    assert!(module.function("Some").is_some());
    let none = module
        .function("None")
        .expect("option.None should exist")
        .call(&[], &mut EvalContext::new())
        .expect("option.None should eval");
    assert_eq!(none["__ctor__"]["__sym__"]["name"], json!("None"));
}
