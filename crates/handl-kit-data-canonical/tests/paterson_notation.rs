//! Tests for Paterson arrow do-notation: `-<`, `<-`, and `returnA`.
//!
//! These constructs extend handl with the surface notation from Ross
//! Paterson's "A New Notation for Arrows" paper:
//!
//! - `f -< x`  — arrow application: apply arrow `f` to input `x`.
//!   Desugars to `call(f, x)`.
//! - `returnA` — the identity arrow. Desugars to `function x ~> x`.
//!   Used as `returnA -< e` to return `e` from a `do` block.
//! - `y <- f -< x` — arrow bind: evaluate `f -< x` and bind the
//!   result to `y` in the current scope. Used inside `do` blocks.

use handl_kit_data_canonical;
use handl_language::Source;
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::EngineRegistry;
use std::rc::Rc;

fn full_runtime() -> handl_runtime::Runtime {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);
    kit.into_runtime(&registry, None)
        .expect("runtime binds engines")
}

// ---------------------------------------------------------------------------
// Token / lexer tests
// ---------------------------------------------------------------------------

#[test]
fn arrow_app_token_exists() {
    let kit = handl_kit_data_canonical::assemble().expect("kit");
    let lang = kit.lang();
    assert!(
        lang.tokens.get_by_id("token.arrow_app").is_some(),
        "token.arrow_app should exist",
    );
}

#[test]
fn arrow_bind_token_exists() {
    let kit = handl_kit_data_canonical::assemble().expect("kit");
    let lang = kit.lang();
    assert!(
        lang.tokens.get_by_id("token.arrow_bind").is_some(),
        "token.arrow_bind should exist",
    );
}

#[test]
fn return_a_keyword_exists() {
    let kit = handl_kit_data_canonical::assemble().expect("kit");
    let lang = kit.lang();
    assert!(
        lang.tokens.get_by_id("token.keyword.returnA").is_some(),
        "token.keyword.returnA should exist",
    );
}

// ---------------------------------------------------------------------------
// Grammar tests
// ---------------------------------------------------------------------------

#[test]
fn arrow_app_grammar_exists() {
    let kit = handl_kit_data_canonical::assemble().expect("kit");
    let lang = kit.lang();
    let row = lang.grammar.get_by_id("grammar.arrow-app.infix");
    assert!(row.is_some(), "grammar.arrow-app.infix should exist");
    let row = row.unwrap();
    assert_eq!(row.token, "token.arrow_app");
    assert_eq!(row.child_shape.node_kind, "arrow_app");
    assert_eq!(row.child_shape.arity, 2);
}

#[test]
fn arrow_bind_grammar_exists() {
    let kit = handl_kit_data_canonical::assemble().expect("kit");
    let lang = kit.lang();
    let row = lang.grammar.get_by_id("grammar.arrow-bind.infix");
    assert!(row.is_some(), "grammar.arrow-bind.infix should exist");
    let row = row.unwrap();
    assert_eq!(row.token, "token.arrow_bind");
    assert_eq!(row.child_shape.node_kind, "arrow_bind");
    assert_eq!(row.child_shape.arity, 2);
}

// ---------------------------------------------------------------------------
// Lexing tests: -< and <- tokenize correctly (not as < minus, etc.)
// ---------------------------------------------------------------------------

#[test]
fn arrow_app_lexes_as_single_token() {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new("f -< x")).expect("lex");
    // Should have: identifier(f), -<, identifier(x), eof
    let kinds: Vec<&str> = toks.0.iter().map(|t| t.kind.as_str()).collect();
    assert!(
        kinds.contains(&"token.arrow_app"),
        "expected token.arrow_app in {:?}",
        kinds,
    );
}

#[test]
fn arrow_bind_lexes_as_single_token() {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new("y <- f")).expect("lex");
    let kinds: Vec<&str> = toks.0.iter().map(|t| t.kind.as_str()).collect();
    assert!(
        kinds.contains(&"token.arrow_bind"),
        "expected token.arrow_bind in {:?}",
        kinds,
    );
}

#[test]
fn return_a_lexes_as_keyword() {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new("returnA")).expect("lex");
    let kinds: Vec<&str> = toks.0.iter().map(|t| t.kind.as_str()).collect();
    assert!(
        kinds.contains(&"token.keyword.returnA"),
        "expected token.keyword.returnA in {:?}",
        kinds,
    );
}

// ---------------------------------------------------------------------------
// Desugar tests
// ---------------------------------------------------------------------------

#[test]
fn arrow_app_desugars_to_call() {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new("f -< x")).expect("lex");
    let ast = rt.parse(&toks).expect("parse");
    // After desugar, no node should have kind "arrow_app" — all should
    // have been rewritten to "call".
    for node in ast.nodes() {
        assert_ne!(
            node.kind, "arrow_app",
            "arrow_app should have been desugared to call",
        );
    }
    // The root should be a "call" node.
    let root = &ast.nodes()[ast.nodes().len() - 1];
    assert_eq!(root.kind, "call");
}

#[test]
fn return_a_desugars_to_identity_function() {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new("returnA")).expect("lex");
    let ast = rt.parse(&toks).expect("parse");
    // After desugar, no node should have kind "returnA".
    for node in ast.nodes() {
        assert_ne!(
            node.kind, "returnA",
            "returnA should have been desugared to function",
        );
    }
    // The root should be a "function" node (identity).
    let root = &ast.nodes()[ast.nodes().len() - 1];
    assert_eq!(root.kind, "function");
}

// ---------------------------------------------------------------------------
// End-to-end eval tests
// ---------------------------------------------------------------------------

#[test]
fn return_a_applied_to_value_returns_value() {
    let rt = full_runtime();
    // returnA -< 42  →  call(identity, 42)  →  42
    let result = rt.run(&Source::new("returnA -< 42")).expect("eval");
    assert_eq!(result, serde_json::json!(42));
}

#[test]
fn arrow_app_calls_closure() {
    let rt = full_runtime();
    // (x ~> x + 1) -< 41  →  call(\x ~> x+1, 41)  →  42
    let result = rt
        .run(&Source::new("(x ~> x + 1) -< 41"))
        .expect("eval");
    assert_eq!(result, serde_json::json!(42));
}

#[test]
fn do_block_with_bind_and_return_a() {
    let rt = full_runtime();
    // do { y <- (x ~> x + 1) -< 41; returnA -< y }
    // → bind y = 42, then return y = 42
    let src = "do { y <- (x ~> x + 1) -< 41; returnA -< y }";
    let result = rt.run(&Source::new(src)).expect("eval");
    assert_eq!(result, serde_json::json!(42));
}

#[test]
fn do_block_with_multiple_binds() {
    let rt = full_runtime();
    // do {
    //   y <- (x ~> x + 1) -< 41;
    //   z <- (x ~> x * 2) -< y;
    //   returnA -< z
    // }
    // → y = 42, z = 84, return 84
    let src = "do { y <- (x ~> x + 1) -< 41; z <- (x ~> x * 2) -< y; returnA -< z }";
    let result = rt.run(&Source::new(src)).expect("eval");
    assert_eq!(result, serde_json::json!(84));
}

#[test]
fn arrow_with_do_block() {
    let rt = full_runtime();
    // n ~> do {
    //   y <- (x ~> x + 1) -< n;
    //   returnA -< y
    // }
    // This evaluates to a closure. Call it with 41 → 42.
    let src = "n ~> do { y <- (x ~> x + 1) -< n; returnA -< y }";
    let closure = rt.run(&Source::new(src)).expect("eval closure");
    assert_eq!(
        closure["__arrow__"],
        serde_json::Value::Bool(true),
        "closure should evaluate correctly",
    );
}

#[test]
fn arrow_app_with_arithmetic_input() {
    let rt = full_runtime();
    // f -< x + 1  should parse as f -< (x + 1), not (f -< x) + 1
    // (x ~> x) -< 40 + 2  →  call(id, 42)  →  42
    let result = rt
        .run(&Source::new("(x ~> x) -< 40 + 2"))
        .expect("eval");
    assert_eq!(result, serde_json::json!(42));
}
