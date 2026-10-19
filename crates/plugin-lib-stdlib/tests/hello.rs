//! Tests for the `hello` table module / package (folded from
//! `plugin-lib-hello`).

use handl_platform::{EvalContext, Module, Plugin};
use handl_plugin_lib_stdlib as stdlib;
use serde_json::json;


/// Install the canonical default language once per test binary (ADR-0128
/// §"The 0.8 seam"): `HandlModule` bridge runtimes resolve it lazily.
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

/// Helper: call the `greet` function on a JSON string arg and compare.
fn check_greet(input: &str, expected: &str) {
    ensure_lang();
    let module = stdlib::module("hello").expect("hello module should exist");
    let func = module.function("greet").expect("greet should exist");
    let result = func
        .call(&[json!(input)], &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("greet failed: {e}"));
    assert_eq!(result, json!(expected));
}

#[test]
fn greet_returns_canonical_greeting() {
    // The greet logic is handl-authored in greet.hnl; this confirms the
    // generic HandlModule loader loads + evaluates it.
    check_greet("world", "Hello, world!");
}

#[test]
fn greet_escapes_special_chars() {
    check_greet("a\"b\\c", "Hello, a\"b\\c!");
}

#[test]
fn greet_with_empty_name() {
    check_greet("", "Hello, !");
}

#[test]
fn module_name_is_hello() {
    assert_eq!(stdlib::module("hello").unwrap().name(), "hello");
}

#[test]
fn plugin_id_and_name_are_preserved() {
    let plugin = stdlib::library("hello").expect("hello package should exist");
    assert_eq!(plugin.id(), "lib.hello");
    assert_eq!(plugin.name(), "lib-hello");
}

#[test]
fn plugin_exposes_declared_capability() {
    let plugin = stdlib::library("hello").expect("hello package should exist");
    let caps = plugin.capabilities();
    assert_eq!(caps.len(), 1);
    assert_eq!(caps[0].as_str(), "hello.greet");
}
