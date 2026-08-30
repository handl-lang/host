//! Parity test for the arith emitter (ADR-0044 Phase 4).
//!
//! Verifies that the data-driven engine path produces the same evaluated
//! values as the existing Rust `ArithRule` for the same logical inputs.
//!
//! ## Desugaring difference
//!
//! The existing `ArithRule` desugars `a + b` to `call(math.add(a, b))` at
//! parse time; eval calls the `math.add` host function. The data-driven path
//! produces `binary.add` with a `Prim{op:"add"}` scheme; the tree-walk
//! engine's built-in `add` primitive computes the same result. Parity is on
//! the **evaluated value**, not the AST node kind.

use handl_emitter_arith::emit;
use handl_emitter_value;
use handl_language::{Ast, AstNode, Language, Token, Tokens};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use serde_json::json;

/// Build a merged language with value + arith emitters (needed for atoms).
fn merged_lang() -> Language {
    let mut lang = handl_emitter_value::emit().unwrap();
    lang.merge(emit().unwrap()).unwrap();
    lang
}

fn toks(parts: &[(&str, &str)]) -> Tokens {
    Tokens(
        parts
            .iter()
            .map(|(k, t)| Token {
                kind: k.to_string(),
                text: t.to_string(),
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Parse parity: structural outcomes match (2-child nodes, correct precedence)
// ---------------------------------------------------------------------------

#[test]
fn parse_parity_add_produces_binary_node_with_two_children() {
    let lang = merged_lang();
    let engine = PrattEngine::new();
    let req = ParserRequest::new(toks(&[
        ("token.number", "1"),
        ("token.plus", "+"),
        ("token.number", "2"),
    ]));
    let ast = engine.parse(&lang, &req).unwrap();

    // 3 nodes: lit(1), lit(2), binary.add(children=[0,1])
    assert_eq!(ast.len(), 3);
    assert_eq!(ast.nodes()[0].kind, "value");
    assert_eq!(ast.nodes()[1].kind, "value");
    assert_eq!(ast.nodes()[2].kind, "binary.add");
    assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));
}

#[test]
fn parse_parity_precedence_mul_binds_tighter_than_add() {
    let lang = merged_lang();
    let engine = PrattEngine::new();
    let req = ParserRequest::new(toks(&[
        ("token.number", "1"),
        ("token.plus", "+"),
        ("token.number", "2"),
        ("token.star", "*"),
        ("token.number", "3"),
    ]));
    let ast = engine.parse(&lang, &req).unwrap();

    // Expected: 1 + (2 * 3)
    // Node 0: value(1), Node 1: value(2), Node 2: value(3)
    // Node 3: binary.mul([1,2]), Node 4: binary.add([0,3])
    assert_eq!(ast.len(), 5);
    assert_eq!(ast.nodes()[3].kind, "binary.mul");
    assert_eq!(ast.nodes()[4].kind, "binary.add");
    assert_eq!(ast.nodes()[4].payload, json!({"children": [0, 3]}));
}

#[test]
fn parse_parity_sub_and_div() {
    let lang = merged_lang();
    let engine = PrattEngine::new();
    let req = ParserRequest::new(toks(&[
        ("token.number", "10"),
        ("token.minus", "-"),
        ("token.number", "4"),
    ]));
    let ast = engine.parse(&lang, &req).unwrap();
    assert_eq!(ast.nodes()[2].kind, "binary.sub");

    let req2 = ParserRequest::new(toks(&[
        ("token.number", "10"),
        ("token.slash", "/"),
        ("token.number", "2"),
    ]));
    let ast2 = engine.parse(&lang, &req2).unwrap();
    assert_eq!(ast2.nodes()[2].kind, "binary.div");
}

// ---------------------------------------------------------------------------
// Eval parity: data-driven values match expected (same as Rust math.* calls)
// ---------------------------------------------------------------------------

#[test]
fn eval_parity_add() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    // Build AST: value(1) + value(2) = binary.add([0, 1])
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(
        AstNode::new("binary.add")
            .with_payload(json!({"children": [0, 1]})),
    );

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // Existing Rust path: math.add(1, 2) = 3
    assert_eq!(result, json!(3));
}

#[test]
fn eval_parity_sub() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(10)));
    ast.push(AstNode::new("value").with_payload(json!(4)));
    ast.push(
        AstNode::new("binary.sub")
            .with_payload(json!({"children": [0, 1]})),
    );

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(6));
}

#[test]
fn eval_parity_mul() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(3)));
    ast.push(AstNode::new("value").with_payload(json!(7)));
    ast.push(
        AstNode::new("binary.mul")
            .with_payload(json!({"children": [0, 1]})),
    );

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(21));
}

#[test]
fn eval_parity_div() {
    let lang = merged_lang();
    let engine = TreeWalkEngine::new();

    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(20)));
    ast.push(AstNode::new("value").with_payload(json!(4)));
    ast.push(
        AstNode::new("binary.div")
            .with_payload(json!({"children": [0, 1]})),
    );

    let result = engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    assert_eq!(result, json!(5));
}

/// Full parse + eval pipeline: `1 + 2 * 3` → 7.
#[test]
fn full_pipeline_precedence() {
    let lang = merged_lang();
    let parse_engine = PrattEngine::new();
    let eval_engine = TreeWalkEngine::new();

    let ast = parse_engine
        .parse(
            &lang,
            &ParserRequest::new(toks(&[
                ("token.number", "1"),
                ("token.plus", "+"),
                ("token.number", "2"),
                ("token.star", "*"),
                ("token.number", "3"),
            ])),
        )
        .unwrap();
    let result = eval_engine.eval(&lang, &EvalRequest::new(ast)).unwrap();
    // 1 + (2 * 3) = 7
    assert_eq!(result, json!(7));
}
