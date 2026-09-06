//! Integration tests for the data-driven lexer engine against the full
//! canonical kit (all Phase 4 emitters, including the updated `tokens` emitter
//! that now emits `LexerRuleDef` fragments).

use handl_kit_data_canonical::assemble;
use handl_language::{Source, Token};
use handl_plugin_engine_lex::{LexerEngine, LexerRequest};
use handl_plugin_engine_lex_pattern::DataLexerEngine;

fn lex(src: &str) -> Vec<Token> {
    let kit = assemble().unwrap();
    let lang = kit.lang();
    let engine = DataLexerEngine::new();
    let req = LexerRequest::new(Source::new(src));
    // These integration tests verify *tokenization* (kind/text) against the
    // canonical kit, not byte spans. The lexer now populates `start`/`end`;
    // zero them out so the expected tokens (built with `..Default::default()`)
    // compare equal. Span population is covered by the lib unit tests.
    engine
        .lex(lang, &req)
        .unwrap()
        .0
        .into_iter()
        .map(|mut t| {
            t.start = 0;
            t.end = 0;
            t
        })
        .collect()
}

#[test]
fn lex_simple_expression() {
    let toks = lex("1 + 2");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.number".into(),
                text: "1".into(),
                ..Default::default()
            },
            Token {
                kind: "token.plus".into(),
                text: "+".into(),
                ..Default::default()
            },
            Token {
                kind: "token.number".into(),
                text: "2".into(),
                ..Default::default()
            },
        ]
    );
}

#[test]
fn lex_bind_token_is_double_colon() {
    let toks = lex("x :: 42");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.identifier".into(),
                text: "x".into(),
                ..Default::default()
            },
            Token {
                kind: "token.bind".into(),
                text: "::".into(),
                ..Default::default()
            },
            Token {
                kind: "token.number".into(),
                text: "42".into(),
                ..Default::default()
            },
        ]
    );
}

#[test]
fn lex_let_binding() {
    let toks = lex("let x = 42");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.let".into(),
                text: "let".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "x".into(),
                ..Default::default()
            },
            Token {
                kind: "token.bind".into(),
                text: "=".into(),
                ..Default::default()
            },
            Token {
                kind: "token.number".into(),
                text: "42".into(),
                ..Default::default()
            },
        ]
    );
}

#[test]
fn lex_string_literal() {
    let toks = lex("\"hello world\"");
    assert_eq!(
        toks,
        vec![Token {
            kind: "token.string".into(),
            text: "\"hello world\"".into(),
            ..Default::default()
        }]
    );
}

#[test]
fn lex_delimiters() {
    let toks = lex("(a, b)");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.lparen".into(),
                text: "(".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "a".into(),
                ..Default::default()
            },
            Token {
                kind: "token.comma".into(),
                text: ",".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "b".into(),
                ..Default::default()
            },
            Token {
                kind: "token.rparen".into(),
                text: ")".into(),
                ..Default::default()
            },
        ]
    );
}

#[test]
fn lex_bool_and_null() {
    let toks = lex("true false null");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.bool".into(),
                text: "true".into(),
                ..Default::default()
            },
            Token {
                kind: "token.bool".into(),
                text: "false".into(),
                ..Default::default()
            },
            Token {
                kind: "token.null".into(),
                text: "null".into(),
                ..Default::default()
            },
        ]
    );
}

#[test]
fn lex_longest_match_operators() {
    let toks = lex("a == b");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.identifier".into(),
                text: "a".into(),
                ..Default::default()
            },
            Token {
                kind: "token.equal".into(),
                text: "==".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "b".into(),
                ..Default::default()
            },
        ]
    );
    // `<=>` (spaceship) is the longest `<`-leading operator.
    let toks = lex("a <=> b");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.identifier".into(),
                text: "a".into(),
                ..Default::default()
            },
            Token {
                kind: "token.spaceship".into(),
                text: "<=>".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "b".into(),
                ..Default::default()
            },
        ]
    );
}

#[test]
fn lex_line_comment_produces_token() {
    // A line comment produces a token.comment (the engine does not discard
    // comments — that is a parse-time concern).
    let toks = lex("-- hello\n42");
    assert_eq!(toks[0].kind, "token.comment");
    assert!(toks[0].text.starts_with("--"));
    assert_eq!(toks[1].kind, "token.number");
}

#[test]
fn lex_unicode_contract_characters() {
    // Contract files use Unicode: `→` (function type arrow), `×` (product
    // type), and Greek letters `α`/`β` (type variables). These must lex
    // without error and produce the same token kinds as their ASCII
    // counterparts (`->` → token.arrow, `*` → token.star).
    let toks = lex("axiom prim.add : (Int × Int) → Int");
    assert_eq!(toks[0].kind, "token.keyword.axiom");
    assert_eq!(toks[0].text, "axiom");
    // `×` reuses token.star.
    let star = toks.iter().find(|t| t.text == "×").unwrap();
    assert_eq!(star.kind, "token.star");
    // `→` reuses token.arrow.
    let arrow = toks.iter().find(|t| t.text == "→").unwrap();
    assert_eq!(arrow.kind, "token.arrow");
}

#[test]
fn lex_unicode_greek_type_variables() {
    // Greek letters lex as identifiers.
    let toks = lex("α β γ");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.identifier".into(),
                text: "α".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "β".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "γ".into(),
                ..Default::default()
            },
        ]
    );
}

#[test]
fn lex_unicode_greek_uppercase_identifiers() {
    // Uppercase Greek letters also lex as identifiers.
    let toks = lex("Ω Λ Δ");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.identifier".into(),
                text: "Ω".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "Λ".into(),
                ..Default::default()
            },
            Token {
                kind: "token.identifier".into(),
                text: "Δ".into(),
                ..Default::default()
            },
        ]
    );
}

#[test]
fn lex_unicode_contract_with_greek_vars() {
    // Full contract line from prim.hnl with Greek type variables.
    let toks = lex("axiom prim.eq : (α × α) → Bool");
    assert_eq!(toks[0].kind, "token.keyword.axiom");
    let alphas: Vec<&Token> = toks.iter().filter(|t| t.text == "α").collect();
    assert_eq!(alphas.len(), 2);
    assert_eq!(alphas[0].kind, "token.identifier");
    let star = toks.iter().find(|t| t.text == "×").unwrap();
    assert_eq!(star.kind, "token.star");
    let arrow = toks.iter().find(|t| t.text == "→").unwrap();
    assert_eq!(arrow.kind, "token.arrow");
}

#[test]
fn lex_decimal_and_minus() {
    // `3.14 -5`: the `token.minus` operator rule (priority 20) is tried before
    // the number rule (priority 40), so `-` is lexed as a minus operator.
    // Unary-minus folding is a parser concern, not a lexer concern.
    let toks = lex("3.14 - 5");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.number".into(),
                text: "3.14".into(),
                ..Default::default()
            },
            Token {
                kind: "token.minus".into(),
                text: "-".into(),
                ..Default::default()
            },
            Token {
                kind: "token.number".into(),
                text: "5".into(),
                ..Default::default()
            },
        ]
    );
}
