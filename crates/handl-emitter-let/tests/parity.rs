//! Parity test for the let emitter (ADR-0044 Phase 4).
//!
//! Verifies that the data-driven tree-walk engine evaluates `let` bindings
//! to the same final values as the existing Rust `AssignRule`/`LetRule`.
//!
//! ## Grammar simplification
//!
//! The Pratt engine cannot parse the full `let name = value;` syntax (it
//! lacks delimited-form support). Parity tests construct `let` ASTs manually
//! with the payload shape `{"name": <str>, "children": [value_idx,
//! body_idx]}`. The eval scheme (`Bind{name:"$payload", value:Child(0),
//! body:Child(1)}`) is the fully migrated part.
//!
//! ## Payload convention difference
//!
//! The existing Rust rule produces `payload:{"name":..., "value":<ast>}`.
//! The data-driven path uses `{"name":..., "children":[...]}`. Both evaluate
//! to the same final `Value`.

use handl_emitter_identifier;
use handl_emitter_let::emit;
use handl_emitter_value;
use handl_language::{Ast, AstNode, Language};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use serde_json::json;

/// Build a merged language with value + identifier + let emitters.
fn merged_lang() -> Language {
    let mut lang = handl_emitter_value::emit().unwrap();
    lang.merge(handl_emitter_identifier::emit().unwrap()).unwrap();
    lang.merge(emit().unwrap()).unwrap();
    lang
}

// ---------------------------------------------------------------------------
// Eval parity: let bindings evaluate correctly
// ---------------------------------------------------------------------------

/// `let x = 42; x` → 42
#[test]
fn eval_parity_let_then_var() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    // Node 0: value(42) — the bound value
    // Node 1: var("x")  — the body (references x)
    // Node 2: let({name:"x", children:[0, 1]}) — the let binding
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
    );

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // Existing Rust path: assign x=42, then read x → 42
    assert_eq!(result, json!(42));
}

/// `let x = 1 + 2; x` → 3 (let with arithmetic value)
#[test]
fn eval_parity_let_with_arith_value() {
    use handl_emitter_arith;

    let mut lang = merged_lang();
    lang.merge(handl_emitter_arith::emit().unwrap()).unwrap();
    let engine = TreeWalkEngine::new();

    // Node 0: value(1)
    // Node 1: value(2)
    // Node 2: binary.add([0, 1]) — the value expression
    // Node 3: var("x") — the body
    // Node 4: let({name:"x", children:[2, 3]})
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    );
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [2, 3]})),
    );

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(3));
}

/// `let x = 42; let y = 8; x + y` → 50 (nested lets)
#[test]
fn eval_parity_nested_lets() {
    use handl_emitter_arith;

    let mut lang = merged_lang();
    lang.merge(handl_emitter_arith::emit().unwrap()).unwrap();
    let engine = TreeWalkEngine::new();

    // Node 0: value(42)
    // Node 1: var("x")       — body of inner let #1
    // Node 2: let x=42; body=var(x)  → children=[0, 1]
    // Node 3: value(8)
    // Node 4: binary.add([1, 3])  — x + y... wait, need y
    // Actually let me build this more carefully.
    //
    // let x = 42 in
    //   let y = 8 in
    //     x + y
    //
    // Node 0: value(42)
    // Node 1: value(8)
    // Node 2: var("x")        — used in add
    // Node 3: var("y")        — used in add
    // Node 4: binary.add([2, 3])  — x + y
    // Node 5: let y=8, body=add   → {name:"y", children:[1, 4]}
    // Node 6: let x=42, body=let_y → {name:"x", children:[0, 5]}
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42))); // 0
    ast.push(AstNode::new("value").with_payload(json!(8))); // 1
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 2
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 3
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [2, 3]})),
    ); // 4
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "y", "children": [1, 4]})),
    ); // 5
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 5]})),
    ); // 6

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(50));
}

/// `let x = 42; 99` → 99 (let value is not returned, body is)
#[test]
fn eval_parity_let_returns_body_not_value() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42))); // 0
    ast.push(AstNode::new("value").with_payload(json!(99))); // 1
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
    ); // 2

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(99));
}
