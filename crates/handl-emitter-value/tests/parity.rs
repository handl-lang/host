//! Parity test for the value emitter (ADR-0044 Phase 4).
//!
//! Verifies that the data-driven engine path produces the same evaluated
//! value as the existing Rust `ValueRule` for the same logical input.
//!
//! ## Payload convention difference
//!
//! The existing `ValueRule` produces `payload:{"value": <v>}` and eval
//! extracts `payload["value"]`. The data-driven Pratt engine produces
//! `payload:<v>` (atom convention) and the tree-walk no-scheme fallback
//! returns `payload` directly. Both evaluate to `<v>`.

use handl_emitter_value::emit;
use handl_language::{Ast, AstNode, Token, Tokens};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use serde_json::json;

fn num_tokens(text: &str) -> Tokens {
    Tokens(vec![Token {
        kind: "token.number".into(),
        text: text.into(),
    }])
}

// ---------------------------------------------------------------------------
// Parse parity: Pratt engine produces a "value" node with the number payload
// ---------------------------------------------------------------------------

#[test]
fn parse_parity_value_produces_value_node_with_number() {
    let lang = emit().unwrap();
    let engine = PrattEngine::new();
    let req = ParserRequest::new(num_tokens("42"));
    let ast = engine.parse(&lang, &req).unwrap();

    assert_eq!(ast.len(), 1);
    let root = &ast.nodes()[0];
    assert_eq!(root.kind, "value");
    // Atom convention: number payload (interpreted from token text).
    assert_eq!(root.payload, json!(42));
}

#[test]
fn parse_parity_value_float() {
    let lang = emit().unwrap();
    let engine = PrattEngine::new();
    let req = ParserRequest::new(num_tokens("3.14"));
    let ast = engine.parse(&lang, &req).unwrap();

    assert_eq!(ast.nodes()[0].kind, "value");
    assert_eq!(ast.nodes()[0].payload, json!(3.14));
}

// ---------------------------------------------------------------------------
// Eval parity: tree-walk no-scheme fallback returns payload directly
// ---------------------------------------------------------------------------

#[test]
fn eval_parity_value_returns_payload_directly() {
    let lang = emit().unwrap();
    let engine = TreeWalkEngine::new();

    // Data-driven path: value node with payload 42 (atom convention).
    // No eval scheme → tree-walk returns payload directly.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    let req = EvalRequest::new(ast);
    let result = engine.eval(&lang, &req).unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn eval_parity_value_string_payload() {
    let lang = emit().unwrap();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!("hello")));
    let req = EvalRequest::new(ast);
    let result = engine.eval(&lang, &req).unwrap();
    assert_eq!(result, json!("hello"));
}

#[test]
fn eval_parity_value_bool_payload() {
    let lang = emit().unwrap();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(true)));
    let req = EvalRequest::new(ast);
    let result = engine.eval(&lang, &req).unwrap();
    assert_eq!(result, json!(true));
}

/// Full parse + eval pipeline: token "42" → value node → eval → 42.
#[test]
fn full_pipeline_value() {
    let lang = emit().unwrap();
    let parse_engine = PrattEngine::new();
    let eval_engine = TreeWalkEngine::new();

    let ast = parse_engine
        .parse(&lang, &ParserRequest::new(num_tokens("42")))
        .unwrap();
    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(42));
}
