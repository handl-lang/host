//! Arrow lowering of object spread entries `{a: 1, ...o, b: 2}`.
//!
//! The AST `object` op field-merges a non-`entry` child (`...expr`) when
//! it evaluates to an object and ignores it otherwise, with later keys
//! winning. The arrow lowerer folds the literal entries into `record` ops
//! and chains them with the spreads through `record_merge`.
//!
//! The all-bare form (`{ x = 1; y = 2 }`, `{...o}` with no entries) is the
//! bare-expression block: the AST evaluates every child and returns the
//! last. That is env-threading, not a record literal, so lowering declines
//! and the engine keeps handling it.

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

fn parse(rt: &Runtime, src: &str) -> handl_language::Ast {
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    rt.parse(&toks).expect("parse+desugar must succeed")
}

/// `src` must lower *and* interpret to `want`, and agree with the bound
/// eval engine — no `lower_ast` decline, no interpreter fallback.
fn assert_lowered(src: &str, want: Value) {
    let rt = runtime();
    let ast = parse(&rt, src);
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

/// `src` must decline to lower (the AST engine owns the semantics).
fn assert_declines(src: &str) {
    let rt = runtime();
    let ast = parse(&rt, src);
    let root = AstRef::new(ast.len() - 1);
    assert!(
        lower_ast(&ast, rt.lang(), root).is_err(),
        "{src:?} should decline"
    );
}

#[test]
fn spread_before_entry_lets_the_entry_win() {
    assert_lowered("do { o :: {a: 1}; {...o, a: 2} }", json!({ "a": 2 }));
}

#[test]
fn spread_after_entry_overwrites_it() {
    assert_lowered("do { o :: {a: 1}; {a: 2, ...o} }", json!({ "a": 1 }));
}

#[test]
fn spread_fields_merge() {
    assert_lowered(
        "do { o :: {a: 1, b: 2}; {c: 3, ...o} }",
        json!({ "a": 1, "b": 2, "c": 3 }),
    );
}

#[test]
fn non_object_spread_is_ignored() {
    assert_lowered("{...5, a: 1}", json!({ "a": 1 }));
}

#[test]
fn multiple_spreads_merge_in_order() {
    // An entry is required for the record form: with no `entry` child at
    // all, `{...a, ...b}` is the bare-expression block (returns the last
    // child), so the literal seed key is what makes this a merge.
    assert_lowered(
        "do { a :: {x: 1, k: \"a\"}; b :: {y: 2, k: \"b\"}; {k: \"lit\", ...a, ...b} }",
        json!({ "x": 1, "y": 2, "k": "b" }),
    );
}

#[test]
fn nested_object_spread() {
    assert_lowered(
        "do { o :: {a: 1}; {inner: {b: 2, ...o}} }",
        json!({ "inner": { "a": 1, "b": 2 } }),
    );
}

#[test]
fn empty_object_still_lowers() {
    assert_lowered("{}", json!({}));
}

#[test]
fn spread_free_object_still_lowers() {
    assert_lowered("{a: 1, b: 2}", json!({ "a": 1, "b": 2 }));
}

#[test]
fn all_bare_object_declines() {
    assert_declines("do { o :: {a: 1}; {...o} }");
}
