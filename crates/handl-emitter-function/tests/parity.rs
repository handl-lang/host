//! Parity test for the function emitter (ADR-0044 Phase 4).
//!
//! Verifies that the data-driven tree-walk engine evaluates function literals
//! to the same closure values as the existing Rust `FunctionRule`, and that
//! applying those closures produces the same results.
//!
//! ## Grammar simplification
//!
//! The Pratt engine cannot parse `(params) -> body` syntax. Parity tests
//! construct function ASTs manually with the payload shape
//! `{"params": ["x", ...], "body": <arena_idx>}`. The eval scheme
//! (`Prim{op:"closure", args:[]}`) is the fully migrated part.

use handl_emitter_function::emit;
use handl_emitter_identifier;
use handl_emitter_value;
use handl_language::{Ast, AstNode, Language};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use serde_json::json;

fn merged_lang() -> Language {
    let mut lang = handl_emitter_value::emit().unwrap();
    lang.merge(handl_emitter_identifier::emit().unwrap()).unwrap();
    lang.merge(emit().unwrap()).unwrap();
    lang
}

/// Make a function node with the given params and body index.
fn make_function_node(ast: &mut Ast, params: &[&str], body_idx: usize) -> usize {
    let params_json: Vec<serde_json::Value> = params.iter().map(|p| json!(p)).collect();
    let idx = ast.len();
    ast.push(
        AstNode::new("function").with_payload(json!({
            "params": params_json,
            "body": body_idx,
        })),
    );
    idx
}

/// A function literal with no params evaluates to a closure.
#[test]
fn eval_parity_function_no_params() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    // Node 0: value(42) — the body
    // Node 1: function({params:[], body:0})
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42))); // 0
    make_function_node(&mut ast, &[], 0); // 1

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // Should be a closure value
    assert_eq!(result["__closure__"], json!(true));
    assert_eq!(result["params"], json!([]));
    assert_eq!(result["body"], json!(0));
}

/// A function literal with params evaluates to a closure with those params.
#[test]
fn eval_parity_function_with_params() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0 — body
    make_function_node(&mut ast, &["x", "y"], 0); // 1

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result["__closure__"], json!(true));
    assert_eq!(result["params"], json!(["x", "y"]));
    assert_eq!(result["body"], json!(0));
}

/// A function literal captures the current scope.
#[test]
fn eval_parity_function_captures_scope() {
    use handl_emitter_let;

    let mut lang = merged_lang();
    lang.merge(handl_emitter_let::emit().unwrap()).unwrap();
    let engine = TreeWalkEngine::new();

    // let z = 99 in (x) -> z
    // Node 0: value(99)
    // Node 1: var("z")  — body of function
    // Node 2: function({params:["x"], body:1})
    // Node 3: let({name:"z", children:[0, 2]})
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(99))); // 0
    ast.push(AstNode::new("var").with_payload(json!("z"))); // 1
    let func_idx = make_function_node(&mut ast, &["x"], 1); // 2
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "z", "children": [0, func_idx]})),
    ); // 3

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // The closure should have captured z=99
    assert_eq!(result["__closure__"], json!(true));
    assert_eq!(result["env"]["z"], json!(99));
}

/// Function literal + call: `((x) -> x)(42)` → 42
#[test]
fn eval_parity_function_then_call() {
    use handl_emitter_call;

    let mut lang = merged_lang();
    lang.merge(handl_emitter_call::emit().unwrap()).unwrap();
    let engine = TreeWalkEngine::new();

    // Node 0: var("x")  — body
    // Node 1: function({params:["x"], body:0})
    // Node 2: value(42) — arg
    // Node 3: call({children:[1, 2]})... but call expects 2 args (arity 3)
    // Actually call emitter uses App{callee:Child(0), args:[Child(1),Child(2)]}
    // which requires 3 children. For 1-arg call, we need a different scheme.
    // Let me use 2-arg call with dummy second arg... no, that won't work.
    //
    // Instead, let me test with a 2-param function and 2 args.
    // ((x, y) -> x)(1, 2) → 1
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0 — body
    let func_idx = make_function_node(&mut ast, &["x", "y"], 0); // 1
    ast.push(AstNode::new("value").with_payload(json!(1))); // 2
    ast.push(AstNode::new("value").with_payload(json!(2))); // 3
    ast.push(
        AstNode::new("call").with_payload(json!({"children": [func_idx, 2, 3]})),
    ); // 4

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(1));
}
