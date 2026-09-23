//! Unit tests for `RewriteEngine`: every `RewriteStep`, every
//! `TransformCondition` (including role-based `KindAt`), priority ordering,
//! first-match-wins, `Fixpoint` termination, native dispatch errors,
//! `PassStats`, `RewriteTrace`, `TopDown` traversal, and `ApplyAt`/`Plan`
//! modes.

use handl_language::{
    Ast, AstNode, AstRef, AstShapeDef, ChildRoleSpec, Language, RewriteStep, TransformCondition,
    TransformDef, TransformRule,
};
use handl_plugin_engine_transform::TransformEngine;
use handl_plugin_engine_transform::native::{NativeRewriteRegistry, RewriteCtx};
use handl_plugin_engine_transform::request::{TransformMode, TransformRequest};
use handl_plugin_engine_transform_rewrite::{RewriteEngine, Traversal};
use serde_json::{Value, json};
use std::sync::Arc;

fn leaf(kind: &str, payload: Value) -> AstNode {
    AstNode::new(kind).with_payload(payload)
}

fn node(kind: &str, children: &[usize]) -> AstNode {
    AstNode::new(kind).with_payload(json!({ "children": children }))
}

fn root(ast: &Ast) -> AstRef {
    AstRef::new(ast.len() - 1)
}

/// A language with shapes `a`, `b`, `c`, `wrap` — `wrap` declares roles
/// `target`/`value` on children 0/1.
fn test_lang() -> Language {
    let mut lang = Language::new();
    for k in ["a", "b", "c", "lit", "var"] {
        lang.merge_ast_fragment(AstShapeDef::new(format!("ast.{k}"), k))
            .unwrap();
    }
    lang.merge_ast_fragment(AstShapeDef::new("ast.wrap", "wrap").with_children(vec![
        ChildRoleSpec {
            role: "target".into(),
            variadic: false,
            optional: false,
        },
        ChildRoleSpec {
            role: "value".into(),
            variadic: false,
            optional: false,
        },
    ]))
    .unwrap();
    lang
}

fn add_rule(lang: &mut Language, def: TransformDef) {
    lang.transforms.merge(def).unwrap();
}

// --- RewriteStep coverage ---------------------------------------------------

#[test]
fn step_keep_and_create() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.keep",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![
                    RewriteStep::Keep(0),
                    RewriteStep::Create {
                        kind: "lit".into(),
                        payload: Some(json!(9)),
                        children: vec![],
                    },
                ],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1))); // 0
    ast.push(node("a", &[0])); // 1
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    let top = out.ast.at(out.root).unwrap();
    assert_eq!(top.kind, "b");
    let children = top.payload["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    // Created leaf carries its payload; rewritten node inherits span (None
    // here — input had none).
    let created = out
        .ast
        .get(AstRef::new(children[1].as_u64().unwrap() as usize))
        .unwrap();
    assert_eq!(created.kind, "lit");
    assert_eq!(created.payload, json!(9));
}

#[test]
fn step_keep_role_resolves_via_shape() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.role",
            "p",
            "wrap",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![RewriteStep::KeepRole("value".into())],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1))); // 0 target
    ast.push(leaf("lit", json!(2))); // 1 value
    ast.push(node("wrap", &[0, 1])); // 2
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    let top = out.ast.at(out.root).unwrap();
    assert_eq!(top.kind, "b");
    let kept = top.payload["children"][0].as_u64().unwrap() as usize;
    let kept_node = out.ast.get(AstRef::new(kept)).unwrap();
    assert_eq!(kept_node.payload, json!(2)); // the `value` child, not `target`
}

#[test]
fn step_keep_role_falls_back_to_null_when_unresolvable() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.role-miss",
            "p",
            "a", // `a` declares no roles
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![RewriteStep::KeepRole("nope".into())],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1)));
    ast.push(node("a", &[0]));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    let top = out.ast.at(out.root).unwrap();
    assert_eq!(top.payload["children"][0], Value::Null);
}

#[test]
fn step_keep_path() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.path",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![RewriteStep::KeepPath(vec![0, 0])],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(5))); // 0
    ast.push(node("c", &[0])); // 1
    ast.push(node("a", &[1])); // 2
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    let top = out.ast.at(out.root).unwrap();
    let kept = top.payload["children"][0].as_u64().unwrap() as usize;
    assert_eq!(out.ast.get(AstRef::new(kept)).unwrap().payload, json!(5));
}

#[test]
fn step_keep_rest() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.rest",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![RewriteStep::KeepRest(1)],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1)));
    ast.push(leaf("lit", json!(2)));
    ast.push(leaf("lit", json!(3)));
    ast.push(node("a", &[0, 1, 2]));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    let top = out.ast.at(out.root).unwrap();
    let children = top.payload["children"].as_array().unwrap();
    assert_eq!(children.len(), 2); // children 1 and 2 kept
}

#[test]
fn step_splice_lifts_grandchildren() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.splice",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![RewriteStep::Splice(0)],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1))); // 0
    ast.push(leaf("lit", json!(2))); // 1
    ast.push(node("c", &[0, 1])); // 2 — the child to splice
    ast.push(node("a", &[2])); // 3
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    let top = out.ast.at(out.root).unwrap();
    assert_eq!(top.kind, "b");
    assert_eq!(top.payload["children"].as_array().unwrap().len(), 2);
}

#[test]
fn step_apply_transforms_child_first() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.inner",
            "p",
            "c",
            TransformRule::Declarative {
                target_kind: "lit".into(),
                children: vec![],
                payload: Some(json!("applied")),
            },
        ),
    );
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.outer",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![RewriteStep::Apply {
                    rule_id: "t.inner".into(),
                    index: 0,
                }],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(node("c", &[])); // 0 — `c` is also rewritten by its own rule
    ast.push(node("a", &[0])); // 1
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    let top = out.ast.at(out.root).unwrap();
    assert_eq!(top.kind, "b");
    let applied = top.payload["children"][0].as_u64().unwrap() as usize;
    let applied_node = out.ast.get(AstRef::new(applied)).unwrap();
    assert_eq!(applied_node.payload, json!("applied"));
    // Both the standalone `c` rewrite and the `Apply` fired.
    assert_eq!(out.stats.per_rule.get("t.inner"), Some(&2));
}

#[test]
fn declarative_payload_is_shallow_patch() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.patch",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![],
                payload: Some(json!({"new_key": 1, "old_key": "over"})),
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(AstNode::new("a").with_payload(json!({"old_key": "orig", "other": 42})));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    let top = out.ast.at(out.root).unwrap();
    // Provided keys overwrite, absent keys preserved, no `children` key.
    assert_eq!(top.payload["new_key"], json!(1));
    assert_eq!(top.payload["old_key"], json!("over"));
    assert_eq!(top.payload["other"], json!(42));
    assert!(top.payload.get("children").is_none());
}

#[test]
fn rewritten_node_inherits_span() {
    use handl_language::Span;
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.span",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(AstNode::new("a").with_span(Span { start: 3, end: 9 }));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    assert_eq!(
        out.ast.at(out.root).unwrap().span,
        Some(Span { start: 3, end: 9 })
    );
}

// --- TransformCondition coverage --------------------------------------------

fn rule_with_when(when: TransformCondition) -> TransformDef {
    TransformDef::new(
        "t.cond",
        "p",
        "a",
        TransformRule::Declarative {
            target_kind: "b".into(),
            children: vec![],
            payload: None,
        },
    )
    .with_when(when)
}

#[test]
fn condition_kind_at_positional() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        rule_with_when(TransformCondition::KindAt {
            index: Some(0),
            role: None,
            kind: "lit".into(),
        }),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1)));
    ast.push(node("a", &[0]));
    ast.push(leaf("var", json!("x")));
    ast.push(node("a", &[2]));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    // Only the first `a` (child kind `lit`) rewrote.
    assert_eq!(out.stats.rewritten, 1);
}

#[test]
fn condition_any_child_kind() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        rule_with_when(TransformCondition::AnyChildKind {
            kind: "lit".into(),
        }),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1)));
    ast.push(leaf("var", json!("x")));
    ast.push(node("a", &[0, 1])); // has a `lit` child at a non-zero index
    ast.push(node("a", &[1])); // no `lit` child
    ast.push(node("a", &[])); // no children at all
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    // Only the `a` holding a `lit` anywhere among its children rewrote.
    assert_eq!(out.stats.rewritten, 1);
}

#[test]
fn condition_kind_at_role() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.rolecond",
            "p",
            "wrap",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![],
                payload: None,
            },
        )
        .with_when(TransformCondition::KindAt {
            index: Some(1), // positional fallback points at the wrong child
            role: Some("target".into()),
            kind: "var".into(),
        }),
    );
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0 — target
    ast.push(leaf("lit", json!(1))); // 1 — value
    ast.push(node("wrap", &[0, 1])); // 2
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    assert_eq!(out.ast.at(out.root).unwrap().kind, "b");
}

#[test]
fn condition_payload_eq_childcount_and_or_not() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        rule_with_when(TransformCondition::And(vec![
            TransformCondition::PayloadEq {
                path: "flag".into(),
                value: json!(true),
            },
            TransformCondition::ChildCount(1),
            TransformCondition::Or(vec![
                TransformCondition::PayloadEq {
                    path: "nested.x".into(),
                    value: json!(1),
                },
                TransformCondition::PayloadEq {
                    path: "nested.y".into(),
                    value: json!(2),
                },
            ]),
            TransformCondition::Not(Box::new(TransformCondition::KindAt {
                index: Some(0),
                role: None,
                kind: "var".into(),
            })),
        ])),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(0)));
    ast.push(AstNode::new("a").with_payload(json!({
        "flag": true, "nested": {"y": 2}, "children": [0]
    })));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    assert_eq!(out.ast.at(out.root).unwrap().kind, "b");
}

#[test]
fn condition_mismatch_passes_through() {
    let mut lang = test_lang();
    add_rule(&mut lang, rule_with_when(TransformCondition::ChildCount(5)));
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    assert_eq!(out.ast.at(out.root).unwrap().kind, "a");
    assert_eq!(out.stats.rewritten, 0);
}

// --- Ordering / stats / trace ------------------------------------------------

#[test]
fn priority_ordering_first_match_wins() {
    let mut lang = test_lang();
    // Same pass+kind, different priorities: lower fires first.
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.hi",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "c".into(),
                children: vec![],
                payload: None,
            },
        )
        .with_priority(10),
    );
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.lo",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![],
                payload: None,
            },
        )
        .with_priority(-5),
    );
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    assert_eq!(out.ast.at(out.root).unwrap().kind, "b"); // t.lo fired first
    assert_eq!(out.stats.per_rule.get("t.lo"), Some(&1));
    assert!(out.stats.per_rule.get("t.hi").is_none());
    // Trace records the rewrite.
    assert_eq!(out.trace.rewrites.len(), 1);
    assert_eq!(out.trace.rewrites[0].rule_id, "t.lo");
    assert_eq!(out.trace.rewrites[0].from, AstRef::new(0));
    assert_eq!(out.trace.new_ref(AstRef::new(0)), Some(out.root));
}

#[test]
fn pass_stats_counts_visited_and_rewritten() {
    let mut lang = test_lang();
    add_rule(&mut lang, rule_with_when(TransformCondition::ChildCount(0)));
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1)));
    ast.push(node("a", &[])); // fires (0 children)
    ast.push(leaf("var", json!("x")));
    let out = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    assert_eq!(out.stats.visited, 3);
    assert_eq!(out.stats.rewritten, 1);
    assert_eq!(out.stats.per_rule.get("t.cond"), Some(&1));
}

#[test]
fn fixpoint_terminates_on_self_rewriting_rule() {
    let mut lang = test_lang();
    // `a` → `a` forever: the fixpoint must stop at `max`, not hang.
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.self",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "a".into(),
                children: vec![],
                payload: Some(json!({"tick": true})),
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let out = RewriteEngine::new()
        .with_traversal(Traversal::Fixpoint { max: 3 })
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    // 3 iterations × 1 rewrite each — never converged, budget honored.
    assert_eq!(out.stats.rewritten, 3);
}

#[test]
fn fixpoint_converges_when_stable() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.once",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let out = RewriteEngine::new()
        .with_traversal(Traversal::Fixpoint { max: 10 })
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    assert_eq!(out.stats.rewritten, 1); // fired once, then stable
    assert_eq!(out.ast.at(out.root).unwrap().kind, "b");
}

#[test]
fn top_down_traversal_rewrites() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.td",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![RewriteStep::Keep(0)],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1)));
    ast.push(node("a", &[0]));
    ast.push(node("a", &[1])); // nested `a` around the `a`
    let out = RewriteEngine::new()
        .with_traversal(Traversal::TopDown)
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap();
    // Outer `a` fires first; its kept child (inner `a`) is then visited and
    // rewritten too.
    let top = out.ast.at(out.root).unwrap();
    assert_eq!(top.kind, "b");
    let inner = top.payload["children"][0].as_u64().unwrap() as usize;
    assert_eq!(out.ast.get(AstRef::new(inner)).unwrap().kind, "b");
}

// --- Native dispatch ---------------------------------------------------------

#[test]
fn native_rule_without_registry_errors() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new("t.nat", "p", "a", TransformRule::Native("nope".into())),
    );
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let err = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap_err();
    assert!(err.to_string().contains("no native registry"));
}

#[test]
fn native_rule_dispatches_by_name() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new("t.nat", "p", "a", TransformRule::Native("mk.b".into())),
    );
    let mut reg = NativeRewriteRegistry::new();
    reg.register(
        "mk.b",
        Box::new(|ctx: &mut RewriteCtx| Ok(AstRef::new(ctx.push_leaf("b", json!("made"))))),
    );
    let engine = RewriteEngine::new().with_native_registry(Arc::new(reg));
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let out = engine.run_pass(&lang, &ast, root(&ast), "p").unwrap();
    let top = out.ast.at(out.root).unwrap();
    assert_eq!(top.kind, "b");
    assert_eq!(top.payload, json!("made"));
}

// --- Modes -------------------------------------------------------------------

#[test]
fn apply_at_is_surgical() {
    let mut lang = test_lang();
    add_rule(&mut lang, rule_with_when(TransformCondition::ChildCount(0)));
    let mut ast = Ast::new();
    ast.push(node("a", &[])); // 0 — should fire
    ast.push(leaf("lit", json!(1))); // 1
    ast.push(node("a", &[])); // 2 — should NOT fire (surgical)
    ast.push(node("c", &[0, 2])); // 3
    let out = RewriteEngine::new()
        .apply_rule(&lang, &ast, AstRef::new(2), "t.cond")
        .unwrap();
    // Only node 2 was rewritten.
    assert_eq!(out.trace.rewrites.len(), 1);
    assert_eq!(out.trace.rewrites[0].from, AstRef::new(2));
    let c = out.ast.at(out.root).unwrap();
    assert_eq!(c.kind, "c");
    let second = c.payload["children"][1].as_u64().unwrap() as usize;
    assert_eq!(out.ast.get(AstRef::new(second)).unwrap().kind, "b");
}

#[test]
fn apply_at_unknown_rule_errors() {
    let lang = test_lang();
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    assert!(
        RewriteEngine::new()
            .apply_rule(&lang, &ast, AstRef::new(0), "no.such")
            .is_err()
    );
}

#[test]
fn plan_mode_proposes_without_rewriting() {
    let mut lang = test_lang();
    add_rule(&mut lang, rule_with_when(TransformCondition::ChildCount(0)));
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let engine = RewriteEngine::new();
    let req = TransformRequest {
        ast: ast.clone(),
        root: root(&ast),
        pass: "p".into(),
        mode: TransformMode::Plan,
        options: Value::Null,
    };
    let out = engine.transform(&lang, &req).unwrap();
    // No new arena: output is the input unchanged; the trace proposes.
    assert_eq!(out.ast, ast);
    assert_eq!(out.trace.rewrites.len(), 1);
    assert_eq!(out.trace.rewrites[0].rule_id, "t.cond");
}

#[test]
fn find_matches_reports_sites() {
    let mut lang = test_lang();
    add_rule(&mut lang, rule_with_when(TransformCondition::ChildCount(0)));
    let mut ast = Ast::new();
    ast.push(node("a", &[])); // 0 — matches
    ast.push(leaf("lit", json!(1))); // 1
    ast.push(node("a", &[1])); // 2 — does not match (1 child)
    ast.push(node("c", &[0, 2])); // 3 — root reaches both `a` nodes
    let matches = RewriteEngine::new().find_matches(&lang, &ast, root(&ast), "p");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].at, AstRef::new(0));
    assert_eq!(matches[0].rule_id, "t.cond");
}

#[test]
fn pipeline_feeds_output_to_input() {
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.p1",
            "p1",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![],
                payload: None,
            },
        ),
    );
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.p2",
            "p2",
            "b",
            TransformRule::Declarative {
                target_kind: "c".into(),
                children: vec![],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let out = RewriteEngine::new()
        .run_pipeline(&lang, &ast, root(&ast), &["p1", "p2"])
        .unwrap();
    assert_eq!(out.ast.at(out.root).unwrap().kind, "c");
    assert_eq!(out.trace.rewrites.len(), 2);
}

// --- Adversarial / fuzz (ADR-0094 "Testing") ---------------------------------

/// Deterministic PRNG (xorshift64) — no new deps, fixed seed table.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn pick(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const FUZZ_KINDS: &[&str] = &["a", "b", "c", "lit", "var", "wrap", "num", "mystery"];
const FUZZ_ROLES: &[&str] = &["target", "value", "nonexistent"];

/// Build a randomized arena: every node's children (when present) index
/// earlier nodes — matching the Pratt arena layout — but kinds, payloads,
/// and child counts are arbitrary. Some nodes carry malformed `children`
/// payloads (non-array, out-of-range indices) to stress the evaluator.
fn random_ast(rng: &mut Rng, n: usize) -> Ast {
    let mut ast = Ast::new();
    for _ in 0..n {
        let kind = FUZZ_KINDS[rng.pick(FUZZ_KINDS.len() as u64) as usize];
        let payload = match rng.pick(5) {
            0 => json!(rng.next()),
            1 => json!({ "children": "not-an-array" }),
            2 => json!({ "children": [usize::MAX, 999_999] }),
            _ => {
                let count = rng.pick(4) as usize;
                let children: Vec<usize> = (0..count)
                    .map(|_| {
                        if ast.len() == 0 {
                            0
                        } else {
                            rng.pick(ast.len() as u64) as usize
                        }
                    })
                    .collect();
                json!({ "children": children, "meta": { "k": kind } })
            }
        };
        ast.push(AstNode::new(kind).with_payload(payload));
    }
    ast
}

/// Generate a random `TransformCondition` of bounded depth.
fn random_condition(rng: &mut Rng, depth: usize) -> TransformCondition {
    let variant = rng.pick(if depth == 0 { 3 } else { 6 });
    match variant {
        0 => TransformCondition::KindAt {
            index: match rng.pick(3) {
                0 => None,
                p => Some(p as usize),
            },
            role: match rng.pick(3) {
                0 => None,
                p => Some(FUZZ_ROLES[p as usize % FUZZ_ROLES.len()].to_string()),
            },
            kind: FUZZ_KINDS[rng.pick(FUZZ_KINDS.len() as u64) as usize].to_string(),
        },
        1 => TransformCondition::PayloadEq {
            path: match rng.pick(4) {
                0 => "meta.k".into(),
                1 => "missing.path".into(),
                2 => format!("children.{}.x", rng.pick(8)),
                _ => "".into(),
            },
            value: match rng.pick(3) {
                0 => json!("a"),
                1 => json!(rng.next()),
                _ => json!(null),
            },
        },
        2 => TransformCondition::ChildCount(rng.pick(6) as usize),
        3 => TransformCondition::Not(Box::new(random_condition(rng, depth - 1))),
        4 => TransformCondition::And(
            (0..rng.pick(3) + 1)
                .map(|_| random_condition(rng, depth - 1))
                .collect(),
        ),
        _ => TransformCondition::Or(
            (0..rng.pick(3) + 1)
                .map(|_| random_condition(rng, depth - 1))
                .collect(),
        ),
    }
}

#[test]
fn fuzzed_conditions_never_panic() {
    // Randomized `TransformCondition`s over random ASTs: the evaluator must
    // never panic (ADR-0094). Fixed seeds keep failures reproducible.
    for seed in [0x9E3779B97F4A7C15u64, 42, 0xDEADBEEF] {
        let mut rng = Rng(seed);
        let ast = random_ast(&mut rng, 64);
        for i in 0..40 {
            let mut lang = test_lang();
            for k in FUZZ_KINDS {
                if lang.ast.get_by_identity(k).is_none() {
                    lang.merge_ast_fragment(AstShapeDef::new(format!("ast.{k}"), *k))
                        .unwrap();
                }
            }
            let kind = FUZZ_KINDS[rng.pick(FUZZ_KINDS.len() as u64) as usize];
            let cond = random_condition(&mut rng, 3);
            add_rule(
                &mut lang,
                TransformDef::new(
                    format!("t.fuzz.{seed}.{i}"),
                    "p",
                    kind,
                    TransformRule::Declarative {
                        target_kind: "b".into(),
                        children: vec![RewriteStep::Keep(0)],
                        payload: None,
                    },
                )
                .with_when(cond),
            );
            // Match-only and a real pass — neither may panic.
            let _ = RewriteEngine::new().find_matches(&lang, &ast, root(&ast), "p");
            let _ = RewriteEngine::new().run_pass(&lang, &ast, root(&ast), "p");
        }
    }
}

#[test]
fn overlapping_same_priority_rules_rejected_at_merge() {
    // Two `TransformDef`s that can both fire on the same kind at the same
    // priority in the same pass are a merge error — never a silent
    // first-pick (ADR-0094 "Determinism"). Identity key: `pass:kind:priority`.
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.first",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "b".into(),
                children: vec![],
                payload: None,
            },
        ),
    );
    let err = lang
        .transforms
        .merge(TransformDef::new(
            "t.second",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "c".into(),
                children: vec![],
                payload: None,
            },
        ))
        .unwrap_err();
    assert!(
        matches!(err, handl_language::LangError::Collision { .. }),
        "expected Collision, got: {err}"
    );
    // A different priority or a different pass is *not* a collision.
    lang.transforms
        .merge(
            TransformDef::new(
                "t.third",
                "p",
                "a",
                TransformRule::Declarative {
                    target_kind: "c".into(),
                    children: vec![],
                    payload: None,
                },
            )
            .with_priority(1),
        )
        .unwrap();
    lang.transforms
        .merge(TransformDef::new(
            "t.other-pass",
            "q",
            "a",
            TransformRule::Declarative {
                target_kind: "c".into(),
                children: vec![],
                payload: None,
            },
        ))
        .unwrap();
}

#[test]
fn fixpoint_infinite_growth_hits_node_budget() {
    // An ever-growing rewrite under `Fixpoint` must hit the node budget as
    // a `LangError` — not hang or exhaust memory (ADR-0094 "Termination
    // discipline").
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new(
            "t.grow",
            "p",
            "a",
            TransformRule::Declarative {
                target_kind: "a".into(),
                children: vec![
                    RewriteStep::Create {
                        kind: "a".into(),
                        payload: None,
                        children: vec![],
                    },
                    RewriteStep::Create {
                        kind: "a".into(),
                        payload: None,
                        children: vec![],
                    },
                ],
                payload: None,
            },
        ),
    );
    let mut ast = Ast::new();
    ast.push(node("a", &[]));
    let err = RewriteEngine::new()
        .with_traversal(Traversal::Fixpoint { max: 100 })
        .with_node_budget(1_000)
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap_err();
    assert!(
        err.to_string().contains("exceeded the node budget"),
        "expected node-budget error, got: {err}"
    );
    assert!(err.to_string().contains("pass 'p'") || err.to_string().contains("'p'"));
}

#[test]
fn transform_errors_carry_pass_rule_and_node_context() {
    // Every transform failure surfaces `{pass, rule_id, node_kind, span}`
    // context (ADR-0094 "Errors & diagnostics").
    use handl_language::Span;
    let mut lang = test_lang();
    add_rule(
        &mut lang,
        TransformDef::new("t.nat", "p", "a", TransformRule::Native("missing".into())),
    );
    let mut ast = Ast::new();
    ast.push(
        AstNode::new("a")
            .with_payload(json!({}))
            .with_span(Span { start: 4, end: 9 }),
    );
    let err = RewriteEngine::new()
        .run_pass(&lang, &ast, root(&ast), "p")
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("pass 'p'"), "missing pass context: {msg}");
    assert!(msg.contains("t.nat"), "missing rule id: {msg}");
    assert!(err.span().is_some(), "missing span: {err:?}");
    match &err {
        handl_language::LangError::Engine { node_kind, .. } => {
            assert_eq!(node_kind.as_deref(), Some("a"));
        }
        other => panic!("expected Engine error, got {other:?}"),
    }
}
