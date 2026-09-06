//! Tests for the arrow notation surface (ADR-0052 Phase C).
//!
//! The arrow notation (`x ~> body`) is the ergonomic authoring layer for
//! arrow calculus. The surface syntax is `x ~> body` (single param) or
//! `[x, y] ~> body` (multi param), which parses via the
//! `grammar.arrow.infix` rule directly as a `function` node.
//!
//! ## Grammar design
//!
//! `~>` is an infix rule (`grammar.arrow.infix`, binding power [31, 32]).
//! The Pratt parser parses a left operand (the param) and a right operand
//! (the body). For a single param, the left is a `var` node; for multiple
//! params, the left is an `array` node containing `var` children. The
//! result is a `function` node with payload `{"children": [param_idx,
//! body_idx]}`.
//!
//! ## Eval scheme
//!
//! `eval.function` produces a closure value via the `closure` prim. The
//! closure captures params and body from the node payload (either
//! `{"params": [...], "body": idx}` rich form or `{"children": [param,
//! body]}` Pratt-parsed form).

use handl_kit_data_canonical;
use handl_language::{Language, Source};
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
fn arrow_token_kind_exists() {
    let lang = full_language();
    assert!(
        lang.tokens.get_by_id("token.arrow").is_some(),
        "token.arrow should exist",
    );
}

#[test]
fn arrow_grammar_row_exists() {
    let lang = full_language();
    let row = lang.grammar.get_by_id("grammar.arrow.infix");
    assert!(row.is_some(), "grammar.arrow.infix should exist");
    let row = row.unwrap();
    assert_eq!(row.token, "token.arrow");
    assert_eq!(row.child_shape.node_kind, "function");
    assert_eq!(row.child_shape.arity, 2);
}

#[test]
fn function_ast_shape_exists() {
    let lang = full_language();
    assert!(
        lang.ast.get_by_identity("function").is_some(),
        "ast shape for node_kind 'function' should exist",
    );
}

#[test]
fn function_eval_scheme_exists() {
    let lang = full_language();
    let def = lang.eval.get_by_identity("function");
    assert!(
        def.is_some(),
        "eval scheme for node_kind 'function' should exist",
    );
}

// ---------------------------------------------------------------------------
// End-to-end parse + eval test
// ---------------------------------------------------------------------------

#[test]
fn arrow_x_to_x_evaluates_to_closure() {
    let rt = full_runtime();
    let result = rt.run(&Source::new("x ~> x")).expect("eval should succeed");
    // The result is a closure value: {"__arrow__": true, "params": ["x"], ...}
    assert_eq!(
        result["__arrow__"],
        serde_json::Value::Bool(true),
        "x ~> x should evaluate to a closure",
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
fn arrow_x_to_x_plus_1_parses_as_function() {
    let rt = full_runtime();
    let src = Source::new("x ~> x + 1");
    let toks = rt.lex(&src).expect("lex should succeed");
    let ast = rt.parse(&toks).expect("parse should succeed");

    // The root node (last in the arena) is a `function` node with 2 children.
    let root_idx = ast.nodes().len() - 1;
    let root = &ast.nodes()[root_idx];
    assert_eq!(root.kind, "function", "root node should be 'function'");
    let children = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("function node should have children array");
    assert_eq!(
        children.len(),
        2,
        "function node should have exactly 2 children",
    );

    // The child 0 is the param (a `var` node for bare `x ~> ...`).
    let param_idx = children[0].as_u64().unwrap() as usize;
    let param_node = &ast.nodes()[param_idx];
    assert_eq!(param_node.kind, "var", "param child should be a 'var' node");
    assert_eq!(
        param_node.payload,
        serde_json::json!("x"),
        "param var payload should be 'x'",
    );
}

#[test]
fn arrow_multi_param_parses_as_function_with_array_params() {
    let rt = full_runtime();
    let src = Source::new("[x, y] ~> x + y");
    let toks = rt.lex(&src).expect("lex should succeed");
    let ast = rt.parse(&toks).expect("parse should succeed");

    // The root node is a `function` node with 2 children: [array, body].
    let root_idx = ast.nodes().len() - 1;
    let root = &ast.nodes()[root_idx];
    assert_eq!(root.kind, "function", "root node should be 'function'");
    let children = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("function node should have children array");
    assert_eq!(children.len(), 2, "function node should have 2 children");

    // Child 0 is an `array` node containing the param vars.
    let param_idx = children[0].as_u64().unwrap() as usize;
    let param_node = &ast.nodes()[param_idx];
    assert_eq!(
        param_node.kind, "array",
        "param child should be an 'array' node"
    );
    let array_children = param_node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("array node should have children");
    assert_eq!(
        array_children.len(),
        2,
        "array should have 2 param children"
    );
}
