//! Integration test: exec-transform erasure + final-artifact lint
//! (ADR-0154 §6).
//!
//! - A crafted artifact carrying a leftover `__former__` application,
//!   an unsolved `__meta__`, a stuck `__neutral__`, a `__type__`
//!   universe index, or an undecided type-level node kind is *rejected*
//!   by the lint before evaluation (stuck is a diagnostic, not silent
//!   unsafety).
//! - A clean artifact — 1-arrows and runtime values — passes and
//!   evaluates unchanged.
//! - `type-annot` nodes erase to their subject at exec (AST-level form
//!   of ADR-0151's annotation erasure); the annotation subtree is never
//!   evaluated.

use handl_language::{
    Ast, AstNode, AstRef, Language, Term, TransformDef, TransformRule, TypeRuleDef, Value,
};
use handl_runtime::artifact_lint::{check_exec_artifact, lint_exec_artifact, rules};
use handl_runtime::exec_erasure_pass::{
    EXEC_ERASURE_PASS, apply_exec_erasure, prepare_exec_artifact,
};
use handl_runtime::Runtime;
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use serde_json::json;

fn node(kind: &str, payload: Value) -> AstNode {
    AstNode::new(kind).with_payload(payload)
}

/// A language carrying the canonical `exec-erasure` rule
/// (`type-annot → subject`), mirroring
/// `transform.exec-erasure.type-annot` in the kit's `transforms.yaml`.
fn erasure_lang() -> Language {
    let mut lang = Language::new();
    lang.transforms
        .merge(TransformDef {
            id: "transform.exec-erasure.type-annot".to_string(),
            pass: EXEC_ERASURE_PASS.to_string(),
            node_kind: "type-annot".to_string(),
            priority: 0,
            when: None,
            rule: TransformRule::Replace { path: vec![0] },
            replaces: vec![],
            requires: vec![],
        })
        .unwrap();
    lang
}

/// A language where `pi` has a checker rule but no exec scheme — the
/// canonical split for type-level-only node kinds.
fn typed_lang() -> Language {
    let mut lang = Language::new();
    lang.types
        .merge(TypeRuleDef::new("type.pi", "pi", Term::constant(json!(null))))
        .unwrap();
    lang
}

// ---------------------------------------------------------------------------
// Lint: rejects crafted stuck/type-level artifacts
// ---------------------------------------------------------------------------

#[test]
fn lint_rejects_former_application() {
    let mut ast = Ast::new();
    ast.push(node(
        "value",
        json!({"__former__": {"sym": {"__sym__": {"handle": 7, "name": "Vec"}}, "args": [{"__type__": 0}]}}),
    ));
    let err = check_exec_artifact(&Language::new(), &ast).unwrap_err();
    assert!(
        err.to_string().contains(rules::FORMER_APPLICATION),
        "expected former-application finding, got: {err}"
    );
}

#[test]
fn lint_rejects_unsolved_meta() {
    let mut ast = Ast::new();
    ast.push(node("value", json!({"__meta__": 3})));
    let findings = lint_exec_artifact(&Language::new(), &ast, AstRef::new(0));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, rules::UNSOLVED_META);
}

#[test]
fn lint_rejects_stuck_neutral() {
    let mut ast = Ast::new();
    ast.push(node(
        "value",
        json!({"__neutral__": {"head": "rigid", "index": 0, "spine": []}}),
    ));
    let err = check_exec_artifact(&Language::new(), &ast).unwrap_err();
    assert!(err.to_string().contains(rules::STUCK_NEUTRAL));
}

#[test]
fn lint_rejects_stuck_type_level_node() {
    // `pi` owns a `lang.types` rule but no `lang.eval` scheme — reaching
    // the runnable artifact it could only hit the no-scheme payload
    // fallback (arena indices as a "value"): undecided → diagnostic.
    let lang = typed_lang();
    let mut ast = Ast::new();
    ast.push(node("value", json!("A")));
    ast.push(node("value", json!("B")));
    ast.push(node("pi", json!({"children": [0, 1]})));
    let err = check_exec_artifact(&lang, &ast).unwrap_err();
    assert!(
        err.to_string().contains(rules::TYPE_LEVEL_NODE),
        "expected type-level-node finding, got: {err}"
    );
}

#[test]
fn lint_ignores_unreachable_stuck_node() {
    // Erasure orphans annotation subtrees in the arena; an unreachable
    // `pi` node is dead code — the lint only walks what exec can reach.
    let lang = typed_lang();
    let mut ast = Ast::new();
    ast.push(node("value", json!(42)));
    ast.push(node("pi", json!({"children": [0, 0]})));
    ast.push(node("value", json!(7)));
    let prepared = prepare_exec_artifact(&lang, &ast).unwrap();
    assert_eq!(prepared.len(), 3, "orphaned nodes stay out of reachability");
}

// ---------------------------------------------------------------------------
// Lint: clean artifact passes, exec unchanged
// ---------------------------------------------------------------------------

#[test]
fn lint_passes_clean_artifact() {
    // 1-arrows + runtime values only: literals, a nominal sym, a tagged
    // type value, a defunctionalized arrow value.
    let mut ast = Ast::new();
    ast.push(node("value", json!(42)));
    ast.push(node("value", json!({"__tag__": "type.number", "__registry__": "handl.canonical"})));
    ast.push(node("value", json!({"__sym__": {"handle": 1, "name": "Int"}})));
    ast.push(node(
        "value",
        json!({"__arrow__": {"params": ["x"], "binds": {}}}),
    ));
    ast.push(node("block", json!({"children": [0, 1, 2, 3]})));
    check_exec_artifact(&Language::new(), &ast).unwrap();
}

#[test]
fn eval_clean_artifact_unchanged() {
    // `rt.eval` on an ordinary artifact still evaluates it — the lint
    // does not change exec behavior.
    let rt = Runtime::new(Language::new()).with_eval_engine(Box::new(TreeWalkEngine::new()));
    let mut ast = Ast::new();
    ast.push(node("value", json!(42)));
    let result = rt.eval(&ast).unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn eval_rejects_crafted_stuck_artifact() {
    // The same stuck artifact, dispatched through `Runtime::eval`:
    // the artifact lint fires before the eval engine ever sees it.
    let rt = Runtime::new(Language::new()).with_eval_engine(Box::new(TreeWalkEngine::new()));
    let mut ast = Ast::new();
    ast.push(node("value", json!({"__former__": {"sym": null, "args": []}})));
    let err = rt.eval(&ast).unwrap_err();
    assert!(err.to_string().contains("artifact lint"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Erasure: type-annot → subject at exec (ADR-0151 generalized)
// ---------------------------------------------------------------------------

#[test]
fn type_annot_erases_to_subject() {
    // [0] 42, [1] `foreign.nope` (errors if ever evaluated), [2] annot
    let lang = erasure_lang();
    let mut ast = Ast::new();
    ast.push(node("value", json!(42)));
    ast.push(node("foreign.nope", json!({"children": []})));
    ast.push(node("type-annot", json!({"children": [0, 1]})));

    let rt = Runtime::new(lang.clone()).with_eval_engine(Box::new(TreeWalkEngine::new()));
    let result = rt.eval(&ast).unwrap();
    // The subject evaluated; the `foreign.nope` annotation was erased,
    // not evaluated (it would have raised `unregistered foreign`).
    assert_eq!(result, json!(42));
}

#[test]
fn erasure_drops_orphaned_annotation_subtree() {
    let lang = erasure_lang();
    let mut ast = Ast::new();
    ast.push(node("value", json!(42)));
    ast.push(node("var", json!("Nat")));
    ast.push(node("type-annot", json!({"children": [0, 1]})));
    let out = apply_exec_erasure(&lang, &ast).unwrap();
    assert_eq!(out.len(), 1, "annotation subtree is dead code — dropped");
    assert_eq!(out.nodes()[0].kind, "value");
    assert_eq!(out.nodes()[0].payload, json!(42));
}

#[test]
fn stdlib_size_module_artifact_passes_lint() {
    // A real stdlib-size artifact: parse `arrow_model.hnl` (the .hnl arrow
    // model — `match` arms, `while`/`rec`, `data`-style payloads) through
    // the canonical kit and run erasure + lint over it. This is the
    // "exec behavior unchanged" gate: the artifact carries clause-pair
    // kinds (`arm`) and runtime values only.
    let kit = handl_kit_prelude::Kit::new().expect("kit");
    let src = include_str!("fixtures/arrow_model.hnl");
    let ast = kit.parse(src).expect("parse");
    let lang = handl_kit_data_prelude::language_from_yaml().unwrap();
    prepare_exec_artifact(&lang, &ast).expect("stdlib-size artifact must lint clean");
}
