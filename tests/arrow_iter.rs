//! Tests for the `while_loop`/`for_loop`/`fix_loop` Term builders
//! (folded from `plugin-lib-arrow-iter`). The composite `*_step` ops are
//! interpreted by the v2 tree-walk engine.

use handl_language::{ArrSource, Ast, AstNode, AstRef, LangError, Language, Term, Value};
use hnl_engine_exec_tree_walk::ScopeStack;
use hnl_engine_exec_tree_walk::v2::V2Interpreter;
use handl_plugin_lib_stdlib::arrow_iter::{fix_loop, for_loop, while_loop};
use hnl_engine::TermData;
use serde_json::json;

/// Convert `TermData` back to `Term` for engine APIs that require `Term`.
fn td(t: TermData) -> Term {
    t.to_term().expect("TermData -> Term conversion")
}

/// Build a native `op` `TermData` with no literal payload.
fn op(name: &str, args: Vec<TermData>) -> TermData {
    TermData::op(name, args, &ArrSource::Native, None)
}

/// Construct a pair value `{"__pair__": [a, b]}`.
fn pair(a: Value, b: Value) -> Value {
    json!({"__pair__": [a, b]})
}

/// Build a minimal one-node AST and evaluate a standalone scheme with
/// a fresh env, passing `None` as incoming.
fn eval_standalone(
    engine: &V2Interpreter,
    scheme: &Term,
    env: &mut ScopeStack,
) -> Result<Value, LangError> {
    let mut ast = Ast::new();
    ast.push(AstNode::new("test").with_payload(json!({})));
    let node = ast.at(AstRef::new(0)).unwrap();
    engine.eval_scheme_v2(
        scheme,
        &ast,
        AstRef::new(0),
        &Language::new(),
        env,
        node,
        None,
    )
}

#[test]
fn while_loop_counts_down_to_zero() {
    // cond = is_pos (incoming > 0), body = dec (incoming - 1).
    // Start at 5: 5 -> 4 -> 3 -> 2 -> 1 -> 0 (cond falsy) -> return 0.
    let engine = V2Interpreter::new();
    let mut env = ScopeStack::new();
    let cond = td(op("is_pos", vec![]));
    let body = td(op("dec", vec![]));
    let s = td(TermData::seq(
        TermData::arr(json!(5)),
        TermData::from_term(&while_loop(cond, body)),
    ));
    let result = eval_standalone(&engine, &s, &mut env).unwrap();
    assert_eq!(result, json!(0));
}

#[test]
fn for_loop_accumulates_increments() {
    // body = inc (incoming + 1). Input = pair(5, 0): run 5 times.
    // 0 -> 1 -> 2 -> 3 -> 4 -> 5 (counter hits 0) -> return 5.
    let engine = V2Interpreter::new();
    let mut env = ScopeStack::new();
    let body = td(op("inc", vec![]));
    let s = td(TermData::seq(
        TermData::arr(pair(json!(5), json!(0))),
        TermData::from_term(&for_loop(body)),
    ));
    let result = eval_standalone(&engine, &s, &mut env).unwrap();
    assert_eq!(result, json!(5));
}

#[test]
fn fix_loop_halves_to_zero() {
    // body = half_floor (incoming / 2, floored). Start at 100.
    // 100 -> 50 -> 25 -> 12 -> 6 -> 3 -> 1 -> 0 -> 0 (fixpoint) -> 0.
    let engine = V2Interpreter::new();
    let mut env = ScopeStack::new();
    let body = td(op("half_floor", vec![]));
    let s = td(TermData::seq(
        TermData::arr(json!(100)),
        TermData::from_term(&fix_loop(body)),
    ));
    let result = eval_standalone(&engine, &s, &mut env).unwrap();
    assert_eq!(result, json!(0));
}

#[test]
fn while_loop_zero_iterations() {
    // cond = is_pos, body = dec. Start at 0: cond falsy immediately.
    let engine = V2Interpreter::new();
    let mut env = ScopeStack::new();
    let cond = td(op("is_pos", vec![]));
    let body = td(op("dec", vec![]));
    let s = td(TermData::seq(
        TermData::arr(json!(0)),
        TermData::from_term(&while_loop(cond, body)),
    ));
    let result = eval_standalone(&engine, &s, &mut env).unwrap();
    assert_eq!(result, json!(0));
}

#[test]
fn for_loop_zero_iterations() {
    // body = inc. Input = pair(0, 99): zero iterations -> return 99.
    let engine = V2Interpreter::new();
    let mut env = ScopeStack::new();
    let body = td(op("inc", vec![]));
    let s = td(TermData::seq(
        TermData::arr(pair(json!(0), json!(99))),
        TermData::from_term(&for_loop(body)),
    ));
    let result = eval_standalone(&engine, &s, &mut env).unwrap();
    assert_eq!(result, json!(99));
}

#[test]
fn fix_loop_already_at_fixpoint() {
    // body = half_floor. Start at 0: 0 -> 0 (fixpoint) -> return 0.
    let engine = V2Interpreter::new();
    let mut env = ScopeStack::new();
    let body = td(op("half_floor", vec![]));
    let s = td(TermData::seq(
        TermData::arr(json!(0)),
        TermData::from_term(&fix_loop(body)),
    ));
    let result = eval_standalone(&engine, &s, &mut env).unwrap();
    assert_eq!(result, json!(0));
}
