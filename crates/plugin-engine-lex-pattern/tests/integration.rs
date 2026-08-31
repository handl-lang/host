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
    engine.lex(lang, &req).unwrap().0
}

#[test]
fn lex_simple_expression() {
    let toks = lex("1 + 2");
    assert_eq!(
        toks,
        vec![
            Token {
                kind: "token.number".into(),
                text: "1".into()
            },
            Token {
                kind: "token.plus".into(),
                text: "+".into()
            },
            Token {
                kind: "token.number".into(),
                text: "2".into()
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
                kind: "token.keyword.let".into(),
                text: "let".into()
            },
            Token {
                kind: "token.identifier".into(),
                text: "x".into()
            },
            Token {
                kind: "token.equal".into(),
                text: "=".into()
            },
            Token {
                kind: "token.number".into(),
                text: "42".into()
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
            text: "\"hello world\"".into()
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
                text: "(".into()
            },
            Token {
                kind: "token.identifier".into(),
                text: "a".into()
            },
            Token {
                kind: "token.comma".into(),
                text: ",".into()
            },
            Token {
                kind: "token.identifier".into(),
                text: "b".into()
            },
            Token {
                kind: "token.rparen".into(),
                text: ")".into()
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
                text: "true".into()
            },
            Token {
                kind: "token.bool".into(),
                text: "false".into()
            },
            Token {
                kind: "token.null".into(),
                text: "null".into()
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
                text: "a".into()
            },
            Token {
                kind: "token.eqeq".into(),
                text: "==".into()
            },
            Token {
                kind: "token.identifier".into(),
                text: "b".into()
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
                text: "a".into()
            },
            Token {
                kind: "token.spaceship".into(),
                text: "<=>".into()
            },
            Token {
                kind: "token.identifier".into(),
                text: "b".into()
            },
        ]
    );
}

#[test]
fn lex_line_comment_produces_token() {
    // A line comment produces a token.comment (the engine does not discard
    // comments — that is a parse-time concern).
    let toks = lex("// hello\n42");
    assert_eq!(toks[0].kind, "token.comment");
    assert!(toks[0].text.starts_with("//"));
    assert_eq!(toks[1].kind, "token.number");
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
                text: "3.14".into()
            },
            Token {
                kind: "token.minus".into(),
                text: "-".into()
            },
            Token {
                kind: "token.number".into(),
                text: "5".into()
            },
        ]
    );
}
