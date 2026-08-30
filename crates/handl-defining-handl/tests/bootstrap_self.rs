//! # bootstrap_self.rs — ADR-0041 Phase 3 self-hosting bootstrap test
//!
//! Tests that a handl **source program** can call `eval(src)` to evaluate
//! handl source under itself. This is the actual "meta is gone" test: the
//! program is parsed from source text, runs through the full
//! `lex → parse → eval` pipeline via `Kit`, and uses `eval` as an ordinary
//! function call. The interpreter is no longer a fixed wall — a handl
//! program can eval handl source under the language it is itself running in.

use handl_kit_canonical::Kit;

/// The `handl-defining-handl.hnl` source file calls `eval("1 + 2")`.
/// Running it through `Kit::run` must produce `3`.
///
/// This proves the self-hosting bootstrap mechanism: a handl program can
/// evaluate handl source under itself via `eval(src)`.
#[test]
fn handl_defining_handl_self_host_bootstrap() {
    let source = include_str!("../data/handl-defining-handl.hnl");
    let kit = Kit::new().expect("kit must assemble");
    let result = kit.run(source).expect("bootstrap must succeed");
    assert_eq!(
        result, serde_json::json!(3),
        "eval('1 + 2') should return 3"
    );
}

/// Direct self-application: `eval("40 + 2")` → `42`.
#[test]
fn eval_self_inline() {
    let kit = Kit::new().expect("kit must assemble");
    let result = kit
        .run(r#"eval("40 + 2")"#)
        .expect("eval self must succeed");
    assert_eq!(result, serde_json::json!(42));
}

/// Nested eval: a program that evals a program that evals a program.
/// `eval("eval(\"1 + 1\")")` → `2`.
///
/// This proves recursive meta-evaluation works: each `eval` spawns a
/// sub-runtime with meta host functions re-attached, so the sub-program
/// can also call `eval`.
#[test]
fn nested_eval_self() {
    let kit = Kit::new().expect("kit must assemble");
    let src = r#"eval("eval(\"1 + 1\")")"#;
    let result = kit.run(src).expect("nested eval must succeed");
    assert_eq!(result, serde_json::json!(2));
}
