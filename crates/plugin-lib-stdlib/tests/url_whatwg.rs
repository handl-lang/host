//! Integration tests for the `url_whatwg` package row.
//!
//! `url_whatwg` is a pure `.hnl` table module now (`lib-url-whatwg`'s
//! flattened single source) — the old Rust `Whatwg` parser type,
//! `DynParser`, and the `url_whatwg_op_registry` are gone (BACKLOG:
//! "Codec/parser registries → `.hnl` object dispatch"). These tests pin
//! the table row: the package caps, `url_whatwg.parse` returning the
//! parsed record, and the `url_whatwg()` parser token.

use std::sync::Arc;

use handl_platform::{EvalContext, Module, Plugin};
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{HandlModule, ModuleResolver};
use handl_plugin_lib_stdlib::{PackageResolver, library, module_with};
use serde_json::json;

/// Install the canonical default language once per test binary (see
/// `json.rs` — the `HandlModule` bridge runtimes resolve it lazily).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

/// Floor-module resolver: native `char` plus the package table — the same
/// resolver shape production passes to `module_with`.
fn floor_resolver() -> Arc<dyn ModuleResolver> {
    struct CharFloor;
    impl ModuleResolver for CharFloor {
        fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
            (name == "char").then(|| Arc::new(CharModule) as Arc<dyn Module + Send + Sync>)
        }
    }
    Arc::new(PackageResolver {
        fallback: Some(Arc::new(CharFloor)),
    })
}

fn table_module() -> HandlModule {
    ensure_lang();
    module_with("url_whatwg", floor_resolver()).expect("url_whatwg module in the package table")
}

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    module
        .function(name)
        .unwrap_or_else(|| panic!("url_whatwg.{name} bound"))
        .call(args, &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("url_whatwg.{name} evaluates: {e}"))
}

#[test]
fn url_whatwg_package_row_exists() {
    let plugin = library("url_whatwg").expect("url_whatwg package should exist");
    assert_eq!(plugin.id(), "lib.url-whatwg");
    assert_eq!(plugin.name(), "lib-url-whatwg");
    let caps = plugin.capabilities();
    assert!(caps.iter().any(|c| c.as_str() == "url_whatwg.parse"));
    assert!(caps.iter().any(|c| c.as_str() == "url_whatwg.url_whatwg"));
}

#[test]
fn parses_http_url_to_a_record() {
    let module = table_module();
    let u = call(
        &module,
        "parse",
        &[json!("https://example.com:8080/path?a=1#frag")],
    );
    assert_eq!(u["scheme"], "https");
    assert_eq!(u["host"], "example.com");
    assert_eq!(u["port"], "8080");
    assert_eq!(u["path"], "/path");
    assert_eq!(u["query"], "a=1");
    assert_eq!(u["fragment"], "frag");
}

#[test]
fn url_whatwg_object_is_the_parser_token() {
    let module = table_module();
    let token = call(&module, "url_whatwg", &[]);
    assert_eq!(token["type"], "parser");
    assert_eq!(token["name"], "url_whatwg");
    assert!(
        token.get("parse").is_some(),
        "the token carries the parse arrow: {token:?}"
    );
}
