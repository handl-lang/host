//! Tests for the `proc` notation surface (ADR-0052 Phase C).
//!
//! `proc` is arrow do-notation — the ergonomic authoring layer for arrow
//! calculus. The surface syntax is `proc x -> body`, which parses as a prefix
//! rule (`grammar.proc.prefix`, arity 1) wrapping a `function` node produced by
//! the existing `x -> body` arrow-infix rule.
//!
//! ## Grammar design
//!
//! `proc` is a prefix rule with arity 1 and `binding_power: [0, 0]`. The Pratt
//! parser parses one operand expression at right-binding-power 0. The operand
//! is `x -> body`, which the existing `grammar.arrow.infix` rule (binding power
//! [31, 32]) consumes as a `function` node. So `proc x -> body` parses as
//! `proc(function(x, body))` — a `proc` node whose single child is a `function`
//! node. This reuses the existing arrow/function infrastructure with no new
//! parsing logic.
//!
//! ## Eval scheme
//!
//! `eval.proc` is `!Child 0` — it evaluates the function child, which produces
//! a closure value (via `eval.function`'s `closure` prim). For this additive v1
//! phase, `proc` is a surface marker: it evaluates to the same closure as the
//! equivalent `function` expression. The full arrow-calculus desugar of `proc`
//! bodies to `arr`/`>>>`/`first`/`Loop`/`App` is future work that depends on v2
//! being the default kernel.

use handl_kit_data_canonical;
use handl_language::{Language, Scheme, Source};
use serde_json::Value;
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::EngineRegistry;
use std::rc::Rc;

/// Assemble the full canonical kit and return its language.
fn full_language() -> Language {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    kit.lang().clone()
}

/// Build a runtime with the canonical language + Pratt parse + tree-walk eval.
fn full_runtime() -> handl_runtime::Runtime {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);
    kit.into_runtime(&registry, None)
        .expect("runtime binds engines")
}

// ---------------------------------------------------------------------------
// Fragment presence tests
// ---------------------------------------------------------------------------

#[test]
fn proc_token_kind_exists() {
    let lang = full_language();
    assert!(
        lang.tokens.get_by_id("token.keyword.proc").is_some(),
        "token.keyword.proc should exist",
    );
}

#[test]
fn proc_grammar_row_exists() {
    let lang = full_language();
    let row = lang.grammar.get_by_id("grammar.proc.prefix");
    assert!(row.is_some(), "grammar.proc.prefix should exist");
    let row = row.unwrap();
    assert_eq!(row.token, "token.keyword.proc");
    assert_eq!(row.child_shape.node_kind, "proc");
    assert_eq!(row.child_shape.arity, 1);
}

#[test]
fn proc_ast_shape_exists() {
    let lang = full_language();
    assert!(
        lang.ast.get_by_identity("proc").is_some(),
        "ast shape for node_kind 'proc' should exist",
    );
}

#[test]
fn proc_eval_scheme_exists_and_is_child_0() {
    let lang = full_language();
    let def = lang.eval.get_by_identity("proc");
    assert!(def.is_some(), "eval scheme for node_kind 'proc' should exist");
    let def = def.unwrap();
    assert_eq!(def.id, "eval.proc");
    assert_eq!(
        def.scheme,
        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
        "eval.proc scheme should be arr(proj_child, lit: 0)",
    );
}

// ---------------------------------------------------------------------------
// End-to-end parse + eval test
// ---------------------------------------------------------------------------

#[test]
fn proc_x_to_x_evaluates_to_closure() {
    let rt = full_runtime();
    let result = rt.run(&Source::new("proc x -> x")).expect("eval should succeed");
    // The result is a closure value: {"__closure__": true, "params": ["x"], ...}
    assert_eq!(
        result["__closure__"],
        serde_json::Value::Bool(true),
        "proc x -> x should evaluate to a closure",
    );
    assert_eq!(
        result["params"],
        serde_json::json!(["x"]),
        "closure should have params ['x']",
    );
}

// ---------------------------------------------------------------------------
// Parse-only test
// ---------------------------------------------------------------------------

#[test]
fn proc_x_to_x_plus_1_parses_as_proc_wrapping_function() {
    let rt = full_runtime();
    let src = Source::new("proc x -> x + 1");
    let toks = rt.lex(&src).expect("lex should succeed");
    let ast = rt.parse(&toks).expect("parse should succeed");

    // The root node (last in the arena) is a `proc` node with one child.
    let root_idx = ast.nodes().len() - 1;
    let root = &ast.nodes()[root_idx];
    assert_eq!(root.kind, "proc", "root node should be 'proc'");
    let children = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("proc node should have children array");
    assert_eq!(children.len(), 1, "proc node should have exactly 1 child");

    // The child is a `function` node with params ["x"].
    let func_idx = children[0].as_u64().expect("child index should be a number") as usize;
    let func_node = &ast.nodes()[func_idx];
    assert_eq!(func_node.kind, "function", "child should be a 'function' node");

    // The function's child 0 is the param (a `var` node for bare `x -> ...`).
    let func_children = func_node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("function node should have children array");
    assert_eq!(func_children.len(), 2, "function node should have 2 children");
    let param_idx = func_children[0].as_u64().unwrap() as usize;
    let param_node = &ast.nodes()[param_idx];
    assert_eq!(param_node.kind, "var", "param child should be a 'var' node");
    assert_eq!(
        param_node.payload,
        serde_json::json!("x"),
        "param var payload should be 'x'",
    );
}
