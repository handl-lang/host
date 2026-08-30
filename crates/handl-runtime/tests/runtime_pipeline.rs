//! Integration test: full `lex → parse → eval` pipeline over a `Language` +
//! `Source` via the `Runtime`.
//!
//! No lex engine crate exists in Phase 3 (ADR-0044 §3 lists
//! `plugin-engine-lex-logos` as a future engine). The lex stage is therefore
//! stubbed/manual: this test manually constructs `Tokens`, feeds them to
//! `Runtime::parse` (Pratt engine), then feeds the resulting `Ast` to
//! `Runtime::eval` (tree-walk engine), and asserts the final `Value`.
//!
//! This exercises the `Runtime` pipeline with real engines bound via
//! `RuntimeConfig` (data-driven engine selection, ADR-0044 §7).

use handl_language::{
    AstNode, AstShapeRef, EvalSchemeDef, GrammarPosition, GrammarRow, Language, Scheme, Source,
    Token, Tokens,
};
use handl_runtime::{Runtime, RuntimeConfig};
use serde_json::json;

/// Build a small arithmetic language: numbers + `+` infix (lbp 10),
/// `*` infix (lbp 20), with eval schemes for `lit` (no scheme → payload)
/// and `binary.add` / `binary.mul` (Prim add/mul, children 0 and 1).
fn arith_lang() -> Language {
    let mut lang = Language::new();
    // Token kinds
    lang.tokens
        .merge(handl_language::TokenKindDef::new("token.number"))
        .unwrap();
    lang.tokens
        .merge(handl_language::TokenKindDef::new("token.plus"))
        .unwrap();
    lang.tokens
        .merge(handl_language::TokenKindDef::new("token.star"))
        .unwrap();

    // Grammar: number prefix (atom, arity 0)
    lang.grammar
        .merge(GrammarRow::new(
            "g.num",
            "token.number",
            GrammarPosition::Prefix,
            AstShapeRef::new("lit", 0),
        ))
        .unwrap();
    // Grammar: + infix (lbp 10, rbp 11)
    lang.grammar
        .merge(
            GrammarRow::new(
                "g.plus",
                "token.plus",
                GrammarPosition::Infix,
                AstShapeRef::new("binary.add", 2),
            )
            .with_binding_power(10, 11),
        )
        .unwrap();
    // Grammar: * infix (lbp 20, rbp 21)
    lang.grammar
        .merge(
            GrammarRow::new(
                "g.star",
                "token.star",
                GrammarPosition::Infix,
                AstShapeRef::new("binary.mul", 2),
            )
            .with_binding_power(20, 21),
        )
        .unwrap();

    // Eval schemes
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.add",
            "binary.add",
            Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
        ))
        .unwrap();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.mul",
            "binary.mul",
            Scheme::prim("mul", vec![Scheme::child(0), Scheme::child(1)]),
        ))
        .unwrap();

    lang
}

fn tok(kind: &str, text: &str) -> Token {
    Token {
        kind: kind.into(),
        text: text.into(),
    }
}

#[test]
fn runtime_pipeline_parse_then_eval_addition() {
    // Build the language and a data-driven runtime config selecting
    // parse = "pratt" and eval = "tree-walk".
    let lang = arith_lang();
    let config = RuntimeConfig::new()
        .with_parse("pratt")
        .with_eval("tree-walk");
    let rt = Runtime::from_config(lang, &config, None).expect("config should resolve");

    // Manually construct tokens for "1 + 2" (lex stage is stubbed — no lex
    // engine exists in Phase 3).
    let tokens = Tokens(vec![
        tok("token.number", "1"),
        tok("token.plus", "+"),
        tok("token.number", "2"),
    ]);

    // parse → Ast
    let ast = rt.parse(&tokens).expect("parse should succeed");
    assert_eq!(ast.len(), 3);
    assert_eq!(ast.nodes()[0].kind, "lit");
    assert_eq!(ast.nodes()[0].payload, json!(1));
    assert_eq!(ast.nodes()[1].kind, "lit");
    assert_eq!(ast.nodes()[1].payload, json!(2));
    assert_eq!(ast.nodes()[2].kind, "binary.add");
    assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));

    // eval → Value
    let result = rt.eval(&ast).expect("eval should succeed");
    assert_eq!(result, json!(3));
}

#[test]
fn runtime_pipeline_parse_then_eval_precedence() {
    let lang = arith_lang();
    let config = RuntimeConfig::new()
        .with_parse("pratt")
        .with_eval("tree-walk");
    let rt = Runtime::from_config(lang, &config, None).expect("config should resolve");

    // Tokens for "1 + 2 * 3" — * binds tighter than +.
    let tokens = Tokens(vec![
        tok("token.number", "1"),
        tok("token.plus", "+"),
        tok("token.number", "2"),
        tok("token.star", "*"),
        tok("token.number", "3"),
    ]);

    let ast = rt.parse(&tokens).expect("parse should succeed");
    // Expected: 1 + (2 * 3) = 7
    // Node 0: lit(1), Node 1: lit(2), Node 2: lit(3),
    // Node 3: binary.mul([1,2]), Node 4: binary.add([0,3])
    assert_eq!(ast.nodes()[3].kind, "binary.mul");
    assert_eq!(ast.nodes()[4].kind, "binary.add");

    let result = rt.eval(&ast).expect("eval should succeed");
    assert_eq!(result, json!(7));
}

#[test]
fn runtime_pipeline_lex_stage_is_not_implemented() {
    // No lex engine exists in Phase 3; the lex stage returns NotImplemented.
    let lang = arith_lang();
    let config = RuntimeConfig::new()
        .with_parse("pratt")
        .with_eval("tree-walk");
    let rt = Runtime::from_config(lang, &config, None).expect("config should resolve");

    let err = rt.lex(&Source::new("1 + 2")).unwrap_err();
    assert_eq!(err, handl_language::LangError::NotImplemented);
}

#[test]
fn runtime_pipeline_run_requires_lex_engine() {
    // run() composes lex → parse → eval. Without a lex engine, it returns
    // NotImplemented from the lex stage.
    let lang = arith_lang();
    let config = RuntimeConfig::new()
        .with_parse("pratt")
        .with_eval("tree-walk");
    let rt = Runtime::from_config(lang, &config, None).expect("config should resolve");

    let err = rt.run(&Source::new("1 + 2")).unwrap_err();
    assert_eq!(err, handl_language::LangError::NotImplemented);
}

#[test]
fn runtime_pipeline_manual_ast_eval() {
    // Verify the full pipeline end-to-end by also building an Ast manually
    // and evaluating it, as a cross-check against the parse+eval path.
    let lang = arith_lang();
    let config = RuntimeConfig::new().with_eval("tree-walk");
    let rt = Runtime::from_config(lang, &config, None).expect("config should resolve");

    // Manually build: 0: lit(10), 1: lit(20), 2: binary.add {children: [0,1]}
    let mut ast = handl_language::Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(10)));
    ast.push(AstNode::new("lit").with_payload(json!(20)));
    ast.push(
        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
    );

    let result = rt.eval(&ast).expect("eval should succeed");
    assert_eq!(result, json!(30));
}
