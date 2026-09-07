//! Parity suite (ADR-0094 §"Testing"): `RewriteEngine::run_pass("desugar")`
//! must produce byte-for-byte identical arenas to the pre-migration
//! `handl_runtime::desugar_pass::apply_desugar`, and desugar must be
//! idempotent: `desugar(desugar(x)) == desugar(x)`.

use handl_kit_data_canonical::language_from_yaml;
use handl_language::{Ast, AstNode, AstRef, Language};
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use serde_json::{json, Value};

fn canonical() -> Language {
    language_from_yaml().expect("canonical language loads")
}

/// Run the engine's `"desugar"` pass.
fn engine_desugar(lang: &Language, ast: &Ast) -> Ast {
    let engine = RewriteEngine::new().with_desugar_natives();
    let root = AstRef::new(ast.len() - 1);
    engine
        .run_pass(lang, ast, root, "desugar")
        .expect("engine desugar failed")
        .ast
}

/// Assert the runtime delegate `apply_desugar` agrees with a direct
/// `run_pass("desugar")` call, and idempotence on both.
fn assert_parity(lang: &Language, ast: &Ast) {
    let delegate = handl_runtime::desugar_pass::apply_desugar(lang, ast)
        .expect("apply_desugar failed");
    let engine = engine_desugar(lang, ast);
    assert_eq!(delegate, engine, "engine output diverged from apply_desugar");
    // Idempotence on both paths.
    let delegate2 = handl_runtime::desugar_pass::apply_desugar(lang, &delegate)
        .expect("apply_desugar failed on re-run");
    let engine2 = engine_desugar(lang, &engine);
    assert_eq!(delegate, delegate2, "apply_desugar is not idempotent");
    assert_eq!(engine, engine2, "engine desugar is not idempotent");
}

fn leaf(kind: &str, payload: Value) -> AstNode {
    AstNode::new(kind).with_payload(payload)
}

fn node(kind: &str, children: &[usize]) -> AstNode {
    AstNode::new(kind).with_payload(json!({ "children": children }))
}

#[test]
fn parity_identity_no_rules() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1)));
    ast.push(leaf("lit", json!(2)));
    ast.push(node("binary.add", &[0, 1]));
    assert_parity(&lang, &ast);
}

#[test]
fn parity_call_json_declarative() {
    let lang = canonical();
    // call { children: [json-template, arg] } → json (declarative rule).
    let mut ast = Ast::new();
    ast.push(leaf("json", json!("hello $0"))); // 0
    ast.push(leaf("lit", json!(42))); // 1
    ast.push(node("call", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_call_non_json_when_declines() {
    let lang = canonical();
    // call { children: [var, arg] } — `when` fails → pass-through, no
    // native fallback.
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("f"))); // 0
    ast.push(leaf("lit", json!(1))); // 1
    ast.push(node("call", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_entry_bracket_key() {
    let lang = canonical();
    // entry { children: [array{[k]}, v] } → entry { paren{k}, v }.
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("k"))); // 0
    ast.push(node("array", &[0])); // 1
    ast.push(leaf("lit", json!(7))); // 2
    ast.push(node("entry", &[1, 2])); // 3
    assert_parity(&lang, &ast);
}

#[test]
fn parity_entry_non_bracket_key() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("k"))); // 0
    ast.push(leaf("lit", json!(7))); // 1
    ast.push(node("entry", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_while() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0
    ast.push(node("block", &[0])); // 1
    ast.push(node("while", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_while_state_passing_guard() {
    let lang = canonical();
    // while { children: [cond, function, ...] } — the state-passing form is
    // NOT desugared (guard), passes through unchanged.
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0
    ast.push(AstNode::new("function").with_payload(json!({
        "params": ["s"],
        "body": 0
    }))); // 1
    ast.push(node("while", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_do_while() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0
    ast.push(node("block", &[0])); // 1
    ast.push(node("do-while", &[1, 0])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_for() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0 pattern
    ast.push(leaf("var", json!("xs"))); // 1 collection
    ast.push(node("block", &[0])); // 2 body
    ast.push(node("for", &[0, 1, 2])); // 3
    assert_parity(&lang, &ast);
}

#[test]
fn parity_nullish() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("a"))); // 0
    ast.push(leaf("lit", json!(0))); // 1
    ast.push(node("nullish", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_opt_chain() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("a"))); // 0
    ast.push(leaf("var", json!("field"))); // 1
    ast.push(node("opt-chain", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_data_structured_payload() {
    let lang = canonical();
    // Hand-built structured-JSON `data` payload (the backward-compat path).
    let mut ast = Ast::new();
    ast.push(AstNode::new("data").with_payload(json!({
        "name": "Nat",
        "constructors": [
            { "name": "Zero", "args": [] },
            { "name": "Succ", "args": [{ "name": "pred", "type": "Nat" }] }
        ]
    })));
    assert_parity(&lang, &ast);
}

#[test]
fn parity_match_no_data_passthrough() {
    let lang = canonical();
    // `match` with no `data` declaration in scope: the native declines and
    // the node passes through with remapped children.
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0 subject
    ast.push(leaf("var", json!("_"))); // 1 pattern
    ast.push(leaf("lit", json!(1))); // 2 body
    ast.push(node("arm", &[1, 2])); // 3
    ast.push(node("block", &[3])); // 4
    ast.push(node("match", &[0, 4])); // 5
    assert_parity(&lang, &ast);
}

#[test]
fn parity_nested_mixed() {
    let lang = canonical();
    // while { children: [nullish{a, b}, block{ for{x, xs, body} }] }
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("a"))); // 0
    ast.push(leaf("lit", json!(0))); // 1
    ast.push(node("nullish", &[0, 1])); // 2 — cond
    ast.push(leaf("var", json!("x"))); // 3
    ast.push(leaf("var", json!("xs"))); // 4
    ast.push(node("block", &[3])); // 5 — for body
    ast.push(node("for", &[3, 4, 5])); // 6 — for
    ast.push(node("block", &[6])); // 7 — while body
    ast.push(node("while", &[2, 7])); // 8
    assert_parity(&lang, &ast);
}
