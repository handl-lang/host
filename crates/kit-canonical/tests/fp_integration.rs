//! Integration tests for the FP-relevant eval schemes: `match`, `handle`,
//! `fanout`, `flow`, `field-access`, and `json`.
//!
//! These tests exercise the full pipeline (source → lex → parse → eval)
//! through the canonical `Kit`, verifying that the schemes are correctly
//! defined in the eval data, correctly parsed by the Pratt parser, and
//! correctly evaluated by the tree-walk engine.

use handl_kit_canonical::Kit;
use handl_language::{Ast, AstNode, Value};
use serde_json::json;

/// Build a `Kit` for testing.
fn kit() -> Kit {
    Kit::new().expect("canonical kit must assemble")
}

/// Debug helper: parse and print the AST.
#[allow(dead_code)]
fn debug_parse(src: &str) {
    let kit = kit();
    match kit.parse(src) {
        Ok(ast) => {
            println!("=== {} ===", src);
            for (i, node) in ast.nodes().iter().enumerate() {
                println!("  [{}] {} {}", i, node.kind, node.payload);
            }
        }
        Err(e) => println!("=== {} === ERROR: {}", src, e),
    }
}

// ---------------------------------------------------------------------------
// fanout (&&&) — Foreign host fn `fn.fanout_fn(a, b)` → [a, b]
// ---------------------------------------------------------------------------

#[test]
fn debug_parse_all() {
    // This test just prints parse output for debugging.
    debug_parse("1 &&& 2");
    debug_parse("1 + 1 &&& 2 * 3");
    debug_parse("1 >>> x -> x + 1");
    debug_parse("1.a");
}

#[test]
fn fanout_eager_pair_from_source() {
    let kit = kit();
    let result = kit.run("1 &&& 2").expect("fanout must eval");
    assert_eq!(result, json!([1, 2]));
}

#[test]
fn fanout_with_expressions() {
    let kit = kit();
    // No parens needed — `+` (bp 60) binds tighter than `&&&` (bp 25).
    let result = kit.run("1 + 1 &&& 2 * 3").expect("fanout must eval");
    assert_eq!(result, json!([2, 6]));
}

// ---------------------------------------------------------------------------
// field-access (.) — Foreign host fn `field_access(target, field)` → target[field]
// ---------------------------------------------------------------------------

#[test]
fn field_access_object_from_manual_ast() {
    // The object/array literal parsers have limitations with comma-separated
    // entries, so test field-access via a manual AST.
    let kit = kit();
    let mut ast = Ast::new();
    // Node 0: object { "a": 1, "b": 2 }.
    let obj_node = AstNode::new("object").with_payload(json!({
        "entries": [
            {"key": "a", "value": 1},
            {"key": "b", "value": 2}
        ]
    }));
    let obj_idx = ast.push(obj_node).index();
    // Node 1: var "a" (the field name).
    let var_node = AstNode::new("var").with_payload(json!("a"));
    let var_idx = ast.push(var_node).index();
    // Node 2: field-access { children: [obj_idx, var_idx] }.
    let fa_node = AstNode::new("field-access").with_payload(json!({
        "children": [obj_idx, var_idx]
    }));
    ast.push(fa_node);
    let result = kit.runtime().eval(&ast).expect("field-access must eval");
    assert_eq!(result, json!(1));
}

#[test]
fn field_access_array_from_manual_ast() {
    let kit = kit();
    let mut ast = Ast::new();
    // Node 0: array [10, 20, 30].
    let arr_node = AstNode::new("array").with_payload(json!({
        "elements": [10, 20, 30]
    }));
    let arr_idx = ast.push(arr_node).index();
    // Node 1: literal 1 (the index).
    let lit_node = AstNode::new("lit").with_payload(json!(1));
    let lit_idx = ast.push(lit_node).index();
    // Node 2: field-access { children: [arr_idx, lit_idx] }.
    let fa_node = AstNode::new("field-access").with_payload(json!({
        "children": [arr_idx, lit_idx]
    }));
    ast.push(fa_node);
    let result = kit.runtime().eval(&ast).expect("field-access must eval");
    assert_eq!(result, json!(20));
}

// ---------------------------------------------------------------------------
// flow (>>>) — App scheme: a >>> b = b(a)
// ---------------------------------------------------------------------------

#[test]
fn flow_applies_closure_to_left() {
    let kit = kit();
    // `1 >>> x -> x + 1` — flow applies the closure to 1, yielding 2.
    // `>>>` (bp 20) binds looser than `->` (bp 31), so this parses as
    // `1 >>> (x -> x + 1)`.
    let result = kit.run("1 >>> x -> x + 1").expect("flow must eval");
    assert_eq!(result, json!(2));
}

// ---------------------------------------------------------------------------
// match — Prim op: pattern matching with arms
// ---------------------------------------------------------------------------

#[test]
fn match_literal_pattern() {
    let kit = kit();
    let result = kit.run("match 1 { 1: 100, 2: 200, _: 300 }")
        .expect("match must eval");
    assert_eq!(result, json!(100));
}

#[test]
fn match_second_arm() {
    let kit = kit();
    let result = kit.run("match 2 { 1: 100, 2: 200, _: 300 }")
        .expect("match must eval");
    assert_eq!(result, json!(200));
}

#[test]
fn match_wildcard_arm() {
    let kit = kit();
    let result = kit.run("match 99 { 1: 100, 2: 200, _: 300 }")
        .expect("match must eval");
    assert_eq!(result, json!(300));
}

#[test]
fn match_binder_binds_scrutinee() {
    let kit = kit();
    // `match 42 { x: x + 1 }` — binder `x` binds 42, body evaluates x + 1 = 43.
    let result = kit.run("match 42 { x: x + 1 }")
        .expect("match with binder must eval");
    assert_eq!(result, json!(43));
}

#[test]
fn match_no_arm_matched_errors() {
    let kit = kit();
    let err = kit.run("match 99 { 1: 100, 2: 200 }")
        .expect_err("match with no matching arm should error");
    assert!(err.to_string().contains("no arm matched"));
}

// ---------------------------------------------------------------------------
// handle — Prim op: evaluate body, dispatch to matching clause
// ---------------------------------------------------------------------------

#[test]
fn handle_dispatches_to_matching_clause() {
    let kit = kit();
    let result = kit.run("handle 1 { 1: 100, 2: 200, _: 300 }")
        .expect("handle must eval");
    assert_eq!(result, json!(100));
}

#[test]
fn handle_no_clause_returns_body() {
    let kit = kit();
    // No clause matches → return the body value as-is.
    let result = kit.run("handle 99 { 1: 100, 2: 200 }")
        .expect("handle with no matching clause returns body");
    assert_eq!(result, json!(99));
}

#[test]
fn handle_binder_binds_body_result() {
    let kit = kit();
    // `handle 42 { x: x * 2 }` — binder `x` binds 42, handler evaluates x * 2 = 84.
    let result = kit.run("handle 42 { x: x * 2 }")
        .expect("handle with binder must eval");
    assert_eq!(result, json!(84));
}

// ---------------------------------------------------------------------------
// json — Prim op: parse a JSON template from payload
// ---------------------------------------------------------------------------

/// Build a manual AST for the `json` prim to test it directly, since the
/// source syntax for `json` may not have a parser rule yet.
#[test]
fn json_prim_parses_literal_string() {
    let kit = kit();
    // Build a manual AST: a `json` node with payload { "expr": "{\"a\": 1}" }.
    let mut ast = Ast::new();
    let json_node = AstNode::new("json")
        .with_payload(json!({ "expr": "{\"a\": 1}" }));
    ast.push(json_node);
    let result = kit.runtime().eval(&ast).expect("json prim must eval");
    assert_eq!(result, json!({"a": 1}));
}

#[test]
fn json_prim_with_placeholder_substitution() {
    let kit = kit();
    // Build a manual AST: a `json` node with expr and args (arena indices).
    // The args reference child nodes that evaluate to the values to substitute.
    let mut ast = Ast::new();
    // Node 0: literal 42 (value to substitute into $0).
    let lit_node = AstNode::new("lit").with_payload(json!(42));
    let lit_idx = ast.push(lit_node).index();
    // Node 1: the json node with expr containing $0 and args referencing node 0.
    let json_node = AstNode::new("json")
        .with_payload(json!({
            "expr": "{\"value\": $0}",
            "args": [lit_idx]
        }));
    ast.push(json_node);
    let result = kit.runtime().eval(&ast).expect("json prim with args must eval");
    assert_eq!(result, json!({"value": 42}));
}
