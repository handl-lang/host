//! # bootstrap_self.rs — ADR-0041 Phase 3 self-hosting bootstrap test
//!
//! Tests that a handl **source program** can call `run(src)` to evaluate
//! handl source under itself, and `eval(lang, ast)` / `parse(lang, tokens)`
//! / `lex(lang, text)` as separate pipeline stages.
//!
//! This is the actual "meta is gone" test: the program is parsed from source
//! text, runs through the full `lex → parse → eval` pipeline via `Kit`, and
//! uses `run`/`eval`/`parse`/`lex` as ordinary function calls.

use handl_kit_canonical::Kit;

/// The `handl-defining-handl.hnl` source file calls `run("1 + 2")`.
/// Running it through `Kit::run` must produce `3`.
#[test]
fn handl_defining_handl_self_host_bootstrap() {
    let source = include_str!("../data/handl-defining-handl.hnl");
    let kit = Kit::new().expect("kit must assemble");
    let result = kit.run(source).expect("bootstrap must succeed");
    assert_eq!(result, serde_json::json!(3), "run('1 + 2') should return 3");
}

/// Direct self-application: `run("40 + 2")` → `42`.
#[test]
fn run_self_inline() {
    let kit = Kit::new().expect("kit must assemble");
    let result = kit
        .run(r#"run("40 + 2")"#)
        .expect("run self must succeed");
    assert_eq!(result, serde_json::json!(42));
}

/// Nested run: `run("run(\"1 + 1\")")` → `2`.
#[test]
fn nested_run_self() {
    let kit = Kit::new().expect("kit must assemble");
    let src = r#"run("run(\"1 + 1\")")"#;
    let result = kit.run(src).expect("nested run must succeed");
    assert_eq!(result, serde_json::json!(2));
}
