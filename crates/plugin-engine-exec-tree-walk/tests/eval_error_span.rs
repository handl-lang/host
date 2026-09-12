//! Phase 10.2 regression test: eval errors must carry source span info.
//!
//! Builds a `binary.add` AST node whose operands are `1` and `true` (a type
//! mismatch) and gives that node an explicit `Span`. The tree-walk engine's
//! `add` handler attaches the current node's span to the type-mismatch error
//! via `with_node`/`engine_node`. This test verifies the resulting
//! [`LangError`] exposes a non-`None` span so diagnostics can point at the
//! offending source location.

use handl_language::{Ast, AstNode, EvalSchemeDef, LangError, Span, Term, Value};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use plugin_engine::{ArrSource, TermData};

/// Convert `TermData` back to `Term` for APIs that require `Term`.
fn td(t: TermData) -> Term {
    t.to_term().expect("TermData -> Term conversion")
}

/// Build a native `op` `TermData` with no literal payload.
fn op(name: &str, args: Vec<TermData>) -> TermData {
    TermData::op(name, args, &ArrSource::Native, None)
}

/// Build a native `op` `TermData` with a literal payload.
fn op_lit(name: &str, args: Vec<TermData>, lit: Value) -> TermData {
    TermData::op(name, args, &ArrSource::Native, Some(lit))
}

/// `arr(proj_child, lit: i)` — reference the i-th child node.
fn pc(i: u64) -> TermData {
    op_lit("proj_child", vec![], Value::from(i))
}

#[test]
fn eval_error_includes_span() {
    // Language with an eval scheme for `binary.add` that projects its two
    // children and applies the native `add` op.
    let mut lang = handl_language::Language::new();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.add",
            "binary.add",
            td(op("add", vec![pc(0), pc(1)])),
        ))
        .unwrap();

    // AST: lit(1), lit(true), binary.add { children: [0, 1] } with a span.
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(Value::from(1)));
    ast.push(AstNode::new("lit").with_payload(Value::Bool(true)));
    ast.push(
        AstNode::new("binary.add")
            .with_payload(serde_json::json!({"children": [0, 1]}))
            .with_span(Span { start: 0, end: 9 }),
    );

    let engine = TreeWalkEngine::new();
    let req = EvalRequest::new(ast);
    let result = engine.eval(&lang, &req);
    assert!(result.is_err(), "1 + true should be a type error");
    let err = result.unwrap_err();
    assert!(
        err.span().is_some(),
        "eval error should include span info, got: {:?}",
        err
    );
    // The span should match the one we attached to the binary.add node.
    assert_eq!(err.span(), Some(Span { start: 0, end: 9 }));
    // Sanity: the error is an Engine variant (not, e.g., NotImplemented).
    assert!(matches!(err, LangError::Engine { .. }));
}
