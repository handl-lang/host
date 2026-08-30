//! Parity test for the call emitter (ADR-0044 Phase 4).
//!
//! Verifies that the data-driven tree-walk engine evaluates function calls
//! to the same final values as the existing Rust `CallRule` + `Apply` rule.
//!
//! ## Grammar simplification
//!
//! The Pratt engine cannot parse comma-separated argument lists. Parity
//! tests construct call ASTs manually with the payload shape
//! `{"children": [callee_idx, arg1_idx, arg2_idx]}`. The eval scheme
//! (`App{callee: Child(0), args: [Child(1), Child(2)]}`) is the fully
//! migrated part.
//!
//! ## Payload convention difference
//!
//! The existing Rust rule produces `payload:{"callee": <ast>, "args":[...]}`.
//! The data-driven path uses `{"children":[...]}`. Both evaluate to the same
//! final `Value`.

use handl_emitter_call::emit;
use handl_emitter_function;
use handl_emitter_identifier;
use handl_emitter_value;
use handl_language::{Ast, AstNode, Language};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use serde_json::json;

/// Build a merged language with value + identifier + function + call emitters.
fn merged_lang() -> Language {
    let mut lang = handl_emitter_value::emit().unwrap();
    lang.merge(handl_emitter_identifier::emit().unwrap()).unwrap();
    lang.merge(handl_emitter_function::emit().unwrap()).unwrap();
    lang.merge(emit().unwrap()).unwrap();
    lang
}

/// Build a closure AST node (function literal) with 2 params.
///
/// The function node payload must contain `params` (array of strings) and
/// `body` (arena index) for the `closure` primitive.
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

// ---------------------------------------------------------------------------
// Eval parity: function calls evaluate correctly
// ---------------------------------------------------------------------------

/// `let f = (x, y) -> x + y; f(1, 2)` → 3
#[test]
fn eval_parity_call_with_closure() {
    use handl_emitter_arith;
    use handl_emitter_let;

    let mut lang = merged_lang();
    lang.merge(handl_emitter_arith::emit().unwrap()).unwrap();
    lang.merge(handl_emitter_let::emit().unwrap()).unwrap();
    let engine = TreeWalkEngine::new();

    // Build: let f = (x, y) -> x + y in f(1, 2)
    //
    // Node 0: var("x")          — body of function (references x)
    // Node 1: var("y")          — used in add
    // Node 2: binary.add([0,1]) — x + y (function body)
    // Node 3: function({params:["x","y"], body:2}) — the closure
    // Node 4: value(1)          — first arg
    // Node 5: value(2)          — second arg
    // Node 6: call({children:[3, 4, 5]}) — f(1, 2)
    // Node 7: let({name:"f", children:[3, 6]}) — let f = <closure> in <call>
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 1
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    ); // 2
    let func_idx = make_function_node(&mut ast, &["x", "y"], 2); // 3
    ast.push(AstNode::new("value").with_payload(json!(1))); // 4
    ast.push(AstNode::new("value").with_payload(json!(2))); // 5
    ast.push(
        AstNode::new("call").with_payload(json!({"children": [func_idx, 4, 5]})),
    ); // 6
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "f", "children": [func_idx, 6]})),
    ); // 7

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // Existing Rust path: f(1, 2) where f = (x,y) -> x+y → 3
    assert_eq!(result, json!(3));
}

/// Direct call without let: `((x, y) -> x + y)(1, 2)` → 3
#[test]
fn eval_parity_direct_call() {
    use handl_emitter_arith;

    let mut lang = merged_lang();
    lang.merge(handl_emitter_arith::emit().unwrap()).unwrap();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 1
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    ); // 2
    let func_idx = make_function_node(&mut ast, &["x", "y"], 2); // 3
    ast.push(AstNode::new("value").with_payload(json!(1))); // 4
    ast.push(AstNode::new("value").with_payload(json!(2))); // 5
    ast.push(
        AstNode::new("call").with_payload(json!({"children": [func_idx, 4, 5]})),
    ); // 6

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(3));
}

/// Call with captured variable: `let z = 10; let f = (x, y) -> x + y + z; f(1, 2)` → 13
#[test]
fn eval_parity_call_with_capture() {
    use handl_emitter_arith;
    use handl_emitter_let;

    let mut lang = merged_lang();
    lang.merge(handl_emitter_arith::emit().unwrap()).unwrap();
    lang.merge(handl_emitter_let::emit().unwrap()).unwrap();
    let engine = TreeWalkEngine::new();

    // Node 0: var("x")
    // Node 1: var("y")
    // Node 2: var("z")        — captured
    // Node 3: binary.add([0,1]) — x + y
    // Node 4: binary.add([3,2]) — (x + y) + z
    // Node 5: function({params:["x","y"], body:4})
    // Node 6: value(1)
    // Node 7: value(2)
    // Node 8: call({children:[5, 6, 7]})
    // Node 9: value(10)
    // Node 10: let({name:"f", children:[5, 8]})
    // Node 11: let({name:"z", children:[9, 10]})
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x"))); // 0
    ast.push(AstNode::new("var").with_payload(json!("y"))); // 1
    ast.push(AstNode::new("var").with_payload(json!("z"))); // 2
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    ); // 3
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [3, 2]})),
    ); // 4
    let func_idx = make_function_node(&mut ast, &["x", "y"], 4); // 5
    ast.push(AstNode::new("value").with_payload(json!(1))); // 6
    ast.push(AstNode::new("value").with_payload(json!(2))); // 7
    ast.push(
        AstNode::new("call").with_payload(json!({"children": [func_idx, 6, 7]})),
    ); // 8
    ast.push(AstNode::new("value").with_payload(json!(10))); // 9
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "f", "children": [func_idx, 8]})),
    ); // 10
    ast.push(
        AstNode::new("let").with_payload(json!({"name": "z", "children": [9, 10]})),
    ); // 11

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // z=10, f=(x,y)->x+y+z, f(1,2) = 1+2+10 = 13
    assert_eq!(result, json!(13));
}
