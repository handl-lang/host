//! Parity tests for the three-child `assign` let-form —
//! `assign { children: [target, value, body] }`, i.e. `x = v in body` —
//! across all three eval engines.
//!
//! The surface grammar only emits the two-child `x = v` form; the
//! optional `body` role (`ast.yaml`: `assign` → `target`/`value`/
//! optional `body`) is exercised by desugared and hand-built ASTs, so
//! these tests construct the node directly and eval it through each
//! engine. The expected semantics (the tree-walk `assign` arm):
//! evaluate `value`, push a fresh scope, bind `target`, evaluate `body`
//! in the extended environment, pop the scope, and return the body
//! result. The two-child form still evaluates and returns only `value`.
//!
//! The ASTs use only `var`/`lit`/`assign`/`block` nodes. `lit` has no
//! eval scheme — every engine's no-scheme fallback returns the payload
//! as a literal — and `var` resolves via `proj_env $payload` (unbound
//! names evaluate to the name itself as a string, which doubles as the
//! scope-cleanup probe: a leaked binding would shadow the probe).

use handl_language::{Ast, AstNode, LangError, Language};
use hnl_engine_exec::{EvalEngine, EvalRequest};
use hnl_engine_exec_bytecode::BytecodeEngine;
use hnl_engine_exec_cranelift::CraneliftEngine;
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use serde_json::{Value, json};

fn canonical_lang() -> Language {
    handl_kit_data_prelude::assemble()
        .expect("canonical kit assembles")
        .lang()
        .clone()
}

fn eval_all(ast: &Ast) -> Vec<(&'static str, Result<Value, LangError>)> {
    let lang = canonical_lang();
    vec![
        (
            "tree-walk",
            TreeWalkEngine::new().eval(&lang, &EvalRequest::new(ast.clone())),
        ),
        (
            "bytecode",
            BytecodeEngine::new().eval(&lang, &EvalRequest::new(ast.clone())),
        ),
        (
            "cranelift",
            CraneliftEngine::new().eval(&lang, &EvalRequest::new(ast.clone())),
        ),
    ]
}

/// `x = 42 in x` — the body evaluates under the fresh binding.
#[test]
fn assign_let_binds_target_in_body_on_all_engines() {
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0: target
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 1: value
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 2: body
    ast.push(AstNode::new("assign").with_payload(json!({ "children": [0, 1, 2] }))); // 3: root
    for (name, result) in eval_all(&ast) {
        let v = result.unwrap_or_else(|e| panic!("{name}: assign let-form failed: {e:?}"));
        assert_eq!(v, json!(42), "{name}: `x = 42 in x` must yield 42");
    }
}

/// `x = 42 in x + 1` — the bound name composes with other ops in `body`.
#[test]
fn assign_let_body_uses_binding_in_expression_on_all_engines() {
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0: target
    ast.push(AstNode::new("lit").with_payload(json!(41))); // 1: value
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 2: add.left
    ast.push(AstNode::new("lit").with_payload(json!(1))); // 3: add.right
    ast.push(AstNode::new("binary.add").with_payload(json!({ "children": [2, 3] }))); // 4: body
    ast.push(AstNode::new("assign").with_payload(json!({ "children": [0, 1, 4] }))); // 5: root
    for (name, result) in eval_all(&ast) {
        let v = result.unwrap_or_else(|e| panic!("{name}: assign let-form failed: {e:?}"));
        assert_eq!(v, json!(42), "{name}: `x = 41 in x + 1` must yield 42");
    }
}

/// The let-scope pops after `body`: a sibling `x` in the enclosing
/// `block` is unbound — `proj_env` yields the name itself as a string,
/// so a leaked binding would return `42` instead of `"x"`.
#[test]
fn assign_let_scope_pops_after_body_on_all_engines() {
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0: target
    ast.push(AstNode::new("lit").with_payload(json!(42))); // 1: value
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 2: body
    ast.push(AstNode::new("assign").with_payload(json!({ "children": [0, 1, 2] }))); // 3: let stmt
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 4: probe
    ast.push(AstNode::new("block").with_payload(json!({ "children": [3, 4] }))); // 5: root
    for (name, result) in eval_all(&ast) {
        let v = result.unwrap_or_else(|e| panic!("{name}: block after let failed: {e:?}"));
        assert_eq!(
            v,
            json!("x"),
            "{name}: the let binding must not leak past its body \
             (a leak would return 42)"
        );
    }
}

/// The two-child form is unchanged: `x = 7` evaluates and returns the
/// value only.
#[test]
fn assign_two_child_still_returns_value_on_all_engines() {
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 0: target
    ast.push(AstNode::new("lit").with_payload(json!(7))); // 1: value
    ast.push(AstNode::new("assign").with_payload(json!({ "children": [0, 1] }))); // 2: root
    for (name, result) in eval_all(&ast) {
        let v = result.unwrap_or_else(|e| panic!("{name}: two-child assign failed: {e:?}"));
        assert_eq!(v, json!(7), "{name}: `y = 7` must yield 7");
    }
}
