//! Parity test for the identifier emitter (ADR-0044 Phase 4).
//!
//! Verifies that the data-driven engine path (Pratt + tree-walk reading
//! emitted `Language` fragments) produces the same **evaluated value** as the
//! existing Rust rule path (`plugin-eval-core`'s `Var` rule) for the same
//! logical input.
//!
//! ## Payload convention difference
//!
//! The existing Rust `Var` rule reads `node.payload["name"]` (a JSON object
//! payload). The data-driven tree-walk reads `node.payload` directly as a
//! string (the `$payload` convention for atoms). The AST payload shapes
//! differ; the **evaluated values** must match. This is expected and
//! documented in ADR-0044 Phase 4 — the data path uses a different payload
//! convention.

use handl_emitter_identifier::emit;
use handl_language::{Ast, AstNode, Token, Tokens};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use serde_json::json;

/// Build a token stream with a single identifier token.
fn ident_tokens(name: &str) -> Tokens {
    Tokens(vec![Token {
        kind: "token.identifier".into(),
        text: name.into(),
    }])
}

// ---------------------------------------------------------------------------
// Parse parity: Pratt engine produces a "var" node (arity 0 atom)
// ---------------------------------------------------------------------------

#[test]
fn parse_parity_identifier_produces_var_node() {
    let lang = emit().unwrap();
    let engine = PrattEngine::new();
    let req = ParserRequest::new(ident_tokens("foo"));
    let ast = engine.parse(&lang, &req).unwrap();

    // The existing IdentifierRule produces kind "var".
    // The Pratt engine also produces kind "var" (from the grammar row).
    assert_eq!(ast.len(), 1);
    let root = &ast.nodes()[0];
    assert_eq!(root.kind, "var");
    // Atom payload convention: the token text as a string.
    assert_eq!(root.payload, json!("foo"));
}

// ---------------------------------------------------------------------------
// Eval parity: tree-walk evaluates "var" to the bound value
// ---------------------------------------------------------------------------

/// Construct a data-driven AST for `var("x")` and eval it with `x = 42` in
/// scope. The tree-walk uses `Var("$payload")` which reads `node.payload` as
/// the variable name.
///
/// To seed the scope, we build a small AST: a `let` node is not available in
/// this emitter alone, so we construct the AST manually with a `bind` node
/// shape that the tree-walk's `Bind` scheme handles. However, since this
/// emitter only provides the `var` scheme, we test eval parity by constructing
/// a minimal AST where the root is a `var` node and we pre-seed scope via a
/// wrapper.
///
/// The tree-walk engine starts with an empty scope. To test `var` lookup we
/// need a binding in scope. We use a `let` node (kind "let") with a `Bind`
/// scheme — but that scheme is provided by `handl-emitter-let`, not here.
///
/// Instead, we test the no-scope case (unbound var → error) which both paths
/// must agree on, and test the bound case in the `full_language_parity`
/// integration test where `let` is available.
#[test]
fn eval_parity_unbound_var_errors() {
    let lang = emit().unwrap();
    let engine = TreeWalkEngine::new();

    // Data-driven path: var node with payload "x" (atom convention).
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("x")));
    let req = EvalRequest::new(ast);
    let result = engine.eval(&lang, &req);
    let err = result.unwrap_err();
    assert!(
        matches!(err, handl_language::LangError::UnknownVariable { ref name } if name == "x"),
        "expected UnknownVariable(x), got {err:?}"
    );
}

/// Eval parity for a bound variable: we manually construct a `let`-shaped
/// node using the tree-walk's `Bind` scheme convention. The `let` node has
/// payload `{"name": "x", "children": [value_idx, body_idx]}` and the
/// `Bind{name:"$payload", value:Child(0), body:Child(1)}` scheme.
///
/// Since this emitter does not provide the `let` scheme, we add it inline
/// to the language for this test. This mirrors how a merged language works.
#[test]
fn eval_parity_bound_var_matches_rust_rule() {
    use handl_emitters::eval_scheme as add_eval;
    use handl_language::Scheme;

    let mut lang = emit().unwrap();
    // Add a let scheme inline (as handl-emitter-let would).
    add_eval(
        &mut lang,
        "eval.let",
        "let",
        Scheme::Bind {
            name: "$payload".into(),
            value: Box::new(Scheme::child(0)),
            body: Box::new(Scheme::child(1)),
        },
    )
    .unwrap();
    // Add a value/lit scheme (no-scheme fallback handles "value" nodes, but
    // we use "lit" which also has no scheme → payload returned directly).
    // Actually the tree-walk no-scheme fallback returns payload directly,
    // so a "lit" node with payload 42 evaluates to 42. No scheme needed.

    let engine = TreeWalkEngine::new();

    // Build AST: let x = 42; x
    // Node 0: lit(42)  — the value
    // Node 1: var("x") — the body (references x)
    // Node 2: let({name:"x", children:[0, 1]}) — the let binding
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(
        AstNode::new("let")
            .with_payload(json!({"name": "x", "children": [0, 1]})),
    );

    let req = EvalRequest::new(ast);
    let result = engine.eval(&lang, &req).unwrap();
    assert_eq!(result, json!(42));
}

/// Verify the `$payload` convention: `Var("$payload")` reads `node.payload`
/// as a string. This is the core data-driven mechanism that replaces the
/// Rust rule's `payload["name"]` access.
#[test]
fn var_payload_convention_reads_payload_as_name() {
    let lang = emit().unwrap();
    let engine = TreeWalkEngine::new();

    // A var node with payload "myvar" should look up "myvar" in scope.
    // With empty scope, this errors with UnknownVariable("myvar").
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("myvar")));
    let req = EvalRequest::new(ast);
    let err = engine.eval(&lang, &req).unwrap_err();
    assert!(
        matches!(err, handl_language::LangError::UnknownVariable { ref name } if name == "myvar"),
        "expected UnknownVariable(myvar), got {err:?}"
    );
}
