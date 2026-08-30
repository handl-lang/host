//! # bootstrap_self.rs — ADR-0041 Phase 3 self-hosting bootstrap test
//!
//! Tests that a handl **source program** can use `self` (an ambient global
//! variable holding the current `Language` value) and call `run(self, text)`
//! / `eval(self, ast)` / `lex(self, text)` / `parse(self, tokens)` as
//! ordinary functions.

use handl_kit_canonical::Kit;

/// `run(self, "1 + 2")` → `3`. The canonical self-hosting bootstrap: a handl
/// program evaluates handl source under itself using the `self` global.
#[test]
fn handl_defining_handl_self_host_bootstrap() {
    let source = include_str!("../data/handl-defining-handl.hnl");
    let kit = Kit::new().expect("kit must assemble");
    let result = kit.run(source).expect("bootstrap must succeed");
    assert_eq!(result, serde_json::json!(3), "run(self, '1 + 2') should return 3");
}

/// `run(self, "40 + 2")` → `42`.
#[test]
fn run_self_inline() {
    let kit = Kit::new().expect("kit must assemble");
    let result = kit
        .run(r#"run(self, "40 + 2")"#)
        .expect("run self must succeed");
    assert_eq!(result, serde_json::json!(42));
}

/// Nested: `run(self, "run(self, \"1 + 1\")")` → `2`.
#[test]
fn nested_run_self() {
    let kit = Kit::new().expect("kit must assemble");
    let src = r#"run(self, "run(self, \"1 + 1\")")"#;
    let result = kit.run(src).expect("nested run must succeed");
    assert_eq!(result, serde_json::json!(2));
}

/// `self` alone evaluates to the current `Language` value (a first-class
/// value carrying the actual `handl_language::Language` data).
#[test]
fn self_global_returns_language_value() {
    let kit = Kit::new().expect("kit must assemble");
    let result = kit.run("self").expect("self must succeed");
    assert!(
        handl_language::is_language_value(&result),
        "self should return a Language value, got: {}",
        result
    );
}

/// Full pipeline from source: `eval(self, parse(self, lex(self, "1 + 2")))` → `3`.
#[test]
fn full_pipeline_from_source() {
    let kit = Kit::new().expect("kit must assemble");
    let src = r#"eval(self, parse(self, lex(self, "1 + 2")))"#;
    let result = kit.run(src).expect("pipeline must succeed");
    assert_eq!(result, serde_json::json!(3));
}
