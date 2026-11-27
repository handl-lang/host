//! `f()` — empty-parens application — parity between the arrow lowering
//! and the bound eval engine.
//!
//! `eval.call` packs the (empty) argument list via `proj_rest` and
//! `apply_app` runs the callee with zero arguments: `f()` on a `() ~>`
//! arrow *calls* it. The arrow lowering used to return the bare bound
//! term for zero-argument env-bound calls — a silent no-op that made
//! `ensure_worker_global()`/`now_ms()` yield the arrow referent instead
//! of running the body in every compiled backend. These tests pin the
//! call semantics: the lowered `Term` must interpret to the same value
//! the bound eval engine produces.

use handl_kit_arrow_lower::lower_ast;
use handl_language::{AstRef, Source, Value};
use handl_runtime::Runtime;
use hnl_engine::term_interpreter::interpret;
use hnl_exec_tree_walk::TreeWalkEngine;
use hnl_tool_lex_pattern::DataLexerEngine;
use hnl_tool_parse_pratt::PrattEngine;
use std::collections::HashMap;

fn canonical_lang() -> handl_language::Language {
    handl_kit_data_prelude::assemble()
        .expect("canonical kit assembles")
        .lang()
        .clone()
}

fn runtime() -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(TreeWalkEngine::new()))
}

/// `src` must lower *and* interpret to the same value the bound eval
/// engine produces — no `lower_ast` decline, no interpreter fallback.
fn assert_lowered(src: &str) {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined {src:?}: {e}"));
    let got = interpret(&term, Value::Null)
        .unwrap_or_else(|e| panic!("term interpreter failed on {src:?}: {e}"));
    let want = rt
        .eval_with_globals(&ast, &HashMap::new())
        .unwrap_or_else(|e| panic!("engine eval failed on {src:?}: {e}"));
    assert_eq!(got, want, "{src:?}");
}

/// The reported bug shape: `ensure_worker_global()` on a `() ~>` binding
/// returned the arrow referent instead of running the body.
#[test]
fn empty_parens_calls_nullary_arrow() {
    assert_lowered("do {\n  f := () ~> 42\n  f()\n}");
}

/// The call *result* binds normally — `g := f()` is `g = 42`, not the
/// arrow value (eval agrees: the referent reading never existed).
#[test]
fn empty_parens_result_binds() {
    assert_lowered("do {\n  f := () ~> 42\n  g := f()\n  g\n}");
}

/// A `() ~>` arrow returning an arrow: `mk()` applies the outer arrow,
/// yielding the inner one for a normal call.
#[test]
fn empty_parens_arrow_producing_arrow() {
    assert_lowered("do {\n  mk := () ~> (n) ~> n + 1\n  inc := mk()\n  inc(4)\n}");
}

/// `f()` inside a nested arrow body — the env-bound callee resolves
/// through the same `lower_call` path.
#[test]
fn empty_parens_inside_nested_arrow() {
    assert_lowered("do {\n  f := () ~> 7\n  apply := () ~> f()\n  apply()\n}");
}
