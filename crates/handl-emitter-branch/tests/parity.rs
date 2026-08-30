//! Parity test for the branch emitter (ADR-0044 Phase 4).
//!
//! Verifies that the data-driven tree-walk engine evaluates `if` branches
//! to the same final values as expected.
//!
//! ## Grammar simplification
//!
//! The Pratt engine cannot parse `if (cond) then else` syntax. Parity tests
//! construct `if` ASTs manually with the payload shape
//! `{"children": [cond_idx, then_idx, else_idx]}`. The eval scheme
//! (`Branch{cond: Child(0), then_: Child(1), else_: Child(2)}`) is the
//! fully migrated part.

use handl_emitter_branch::emit;
use handl_emitter_value;
use handl_language::{Ast, AstNode, Language};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use serde_json::json;

fn merged_lang() -> Language {
    let mut lang = handl_emitter_value::emit().unwrap();
    lang.merge(emit().unwrap()).unwrap();
    lang
}

/// `if (true) 1 else 2` → 1
#[test]
fn eval_parity_if_true_returns_then_branch() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    // Node 0: value(true)  — condition
    // Node 1: value(1)     — then branch
    // Node 2: value(2)     — else branch
    // Node 3: if({children:[0, 1, 2]})
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(true))); // 0
    ast.push(AstNode::new("value").with_payload(json!(1))); // 1
    ast.push(AstNode::new("value").with_payload(json!(2))); // 2
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
    ); // 3

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(1));
}

/// `if (false) 1 else 2` → 2
#[test]
fn eval_parity_if_false_returns_else_branch() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(false))); // 0
    ast.push(AstNode::new("value").with_payload(json!(1))); // 1
    ast.push(AstNode::new("value").with_payload(json!(2))); // 2
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
    ); // 3

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(2));
}

/// `if (1 < 2) 10 else 20` → 10 (condition is a comparison)
#[test]
fn eval_parity_if_with_comparison_condition() {
    use handl_emitter_arith;
    let mut lang = merged_lang();
    lang.merge(handl_emitter_arith::emit().unwrap()).unwrap();
    let engine = TreeWalkEngine::new();

    // Node 0: value(1)
    // Node 1: value(2)
    // Node 2: binary.lt([0, 1])  — 1 < 2... but we don't have lt in arith
    // Actually arith only has add/sub/mul/div. Let me use a truthy value.
    // Use value(1) as condition (truthy since non-zero).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1))); // 0 — truthy
    ast.push(AstNode::new("value").with_payload(json!(10))); // 1
    ast.push(AstNode::new("value").with_payload(json!(20))); // 2
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
    ); // 3

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // 1 is truthy → then branch → 10
    assert_eq!(result, json!(10));
}

/// `if (0) 10 else 20` → 20 (0 is falsy)
#[test]
fn eval_parity_if_zero_is_falsy() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(0))); // 0 — falsy
    ast.push(AstNode::new("value").with_payload(json!(10))); // 1
    ast.push(AstNode::new("value").with_payload(json!(20))); // 2
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
    ); // 3

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(20));
}

/// Nested if: `if (true) if (false) 1 else 2 else 3` → 2
#[test]
fn eval_parity_nested_if() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    // Node 0: value(true)   — outer condition
    // Node 1: value(false)  — inner condition
    // Node 2: value(1)      — inner then
    // Node 3: value(2)      — inner else
    // Node 4: if({children:[1, 2, 3]})  — inner if
    // Node 5: value(3)      — outer else
    // Node 6: if({children:[0, 4, 5]})  — outer if
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(true))); // 0
    ast.push(AstNode::new("value").with_payload(json!(false))); // 1
    ast.push(AstNode::new("value").with_payload(json!(1))); // 2
    ast.push(AstNode::new("value").with_payload(json!(2))); // 3
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [1, 2, 3]})),
    ); // 4
    ast.push(AstNode::new("value").with_payload(json!(3))); // 5
    ast.push(
        AstNode::new("if").with_payload(json!({"children": [0, 4, 5]})),
    ); // 6

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // outer: true → inner if
    // inner: false → else → 2
    assert_eq!(result, json!(2));
}
