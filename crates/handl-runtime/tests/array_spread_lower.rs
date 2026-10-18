//! Arrow lowering of array spread elements `[a, ...xs, b]`.
//!
//! The AST `array` op flattens a `rest`-roled child (`...expr`) into the
//! result when it evaluates to an array and pushes it as-is otherwise. The
//! arrow lowerer cannot express that with a single `array` op (its
//! arguments are always plain elements), so it folds the elements into a
//! `concat` chain: `[a, ...xs, b]` → `concat(concat([a], xs), [b])`.
//!
//! Every case must lower, interpret (no `lower_ast` decline, no
//! interpreter fallback), and agree with the bound eval engine.

use std::collections::HashMap;

use handl_kit_arrow_lower::lower_ast;
use handl_language::{AstRef, Source, Value};
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use hnl_engine_lex_pattern::DataLexerEngine;
use hnl_engine_parse_pratt::PrattEngine;
use handl_runtime::Runtime;
use hnl_engine::term_interpreter::interpret;
use serde_json::json;

fn runtime() -> Runtime {
    let lang = handl_kit_data_canonical::assemble()
        .expect("canonical kit assembles")
        .lang()
        .clone();
    Runtime::new(lang)
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(TreeWalkEngine::new()))
}

/// `src` must lower *and* interpret to the same value the bound eval
/// engine produces — no `lower_ast` decline, no interpreter fallback.
fn assert_lowered(src: &str, want: Value) {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined {src:?}: {e}"));
    let got = interpret(&term, Value::Null)
        .unwrap_or_else(|e| panic!("term interpreter failed on {src:?}: {e}"));
    assert_eq!(got, want, "lowered value for {src:?}");
    let engine = rt
        .eval_with_globals(&ast, &HashMap::new())
        .unwrap_or_else(|e| panic!("engine eval failed on {src:?}: {e}"));
    assert_eq!(got, engine, "lowered vs engine for {src:?}");
}

#[test]
fn literal_spread() {
    assert_lowered("[1, ...[2, 3], 4]", json!([1, 2, 3, 4]));
}

#[test]
fn spread_of_bound_variable() {
    assert_lowered("do { xs :: [2, 3]; [1, ...xs, 4] }", json!([1, 2, 3, 4]));
}

#[test]
fn multiple_spreads() {
    // Spread sources arrive as arrow parameters (a tuple), which keeps
    // the test independent of the `do`-block let-encoding.
    assert_lowered(
        "do { f :: (xs, ys) ~> [...xs, ...ys]; f([2], [3, 4]) }",
        json!([2, 3, 4]),
    );
}

#[test]
fn leading_and_trailing_spreads() {
    assert_lowered(
        "do { f :: (xs, ys) ~> [...xs, 3, ...ys]; f([1, 2], [4, 5]) }",
        json!([1, 2, 3, 4, 5]),
    );
}

#[test]
fn empty_spread_contributes_nothing() {
    assert_lowered("[0, ...[], 9]", json!([0, 9]));
}

#[test]
fn non_array_spread_is_pushed_as_is() {
    // The AST `array` op pushes a non-array rest value as a single
    // element; `concat` mirrors that.
    assert_lowered("do { n :: 5; [1, ...n, 2] }", json!([1, 5, 2]));
}

#[test]
fn leading_non_array_spread_is_wrapped() {
    assert_lowered("do { n :: 5; [...n, 2] }", json!([5, 2]));
}

#[test]
fn sole_non_array_spread_is_wrapped() {
    assert_lowered("do { n :: 5; [...n] }", json!([5]));
}

#[test]
fn spread_only_array() {
    assert_lowered("do { xs :: [1, 2]; [...xs] }", json!([1, 2]));
}

#[test]
fn spread_of_computed_expression() {
    assert_lowered("[1, ...[2 + 3], 6]", json!([1, 5, 6]));
}

#[test]
fn nested_spread_arrays() {
    assert_lowered("[[1, ...[2, 3]]]", json!([[1, 2, 3]]));
}

#[test]
fn lower_emits_concat_chain() {
    // The lowering (not the AST fallback) must be what runs: the emitted
    // term carries the `concat` op that flattens the spread.
    let rt = runtime();
    let src = "do { xs :: [2, 3]; [1, ...xs, 4] }";
    let toks = rt.lex(&Source::new(src)).expect("lex");
    let ast = rt.parse(&toks).expect("parse");
    let term = lower_ast(&ast, rt.lang(), AstRef::new(ast.len() - 1)).expect("must lower");
    // The block lowers to an applied quoted arrow, so the `concat` ops sit
    // inside an `Arr` record — match the name textually rather than
    // expecting a bare `Term::Op`.
    let debug = format!("{term:?}");
    assert!(
        debug.contains("concat"),
        "expected a concat op in the lowered term:\n{debug}"
    );
}
