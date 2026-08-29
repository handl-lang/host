#![allow(non_snake_case)]
use handl_plugin_grammar_textmate::{GrammarContributor, GrammarRule};
use handl_plugin_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical string tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum StringKind {
    /// A string token.
    String(String),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for StringKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StringKind::String(v) => write!(f, "string({:?})", v),
            StringKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for StringKind {
    fn is_eof(&self) -> bool {
        matches!(self, StringKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

pub fn String_token(value: String, span: Span) -> Token<StringKind> {
    Token::new(StringKind::String(value), span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<StringKind> for TokenFactory {
    fn eof_kind(&self) -> StringKind {
        StringKind::Eof
    }
}

impl GrammarContributor for TokenFactory {
    fn grammar_rules(&self) -> Vec<GrammarRule> {
        vec![GrammarRule {
            name: "strings".to_string(),
            scope: None,
            r#match: None,
            begin: None,
            end: None,
            captures: Vec::new(),
            patterns: vec![
                GrammarRule::begin_end_rule(
                    "string_double",
                    "string.quoted",
                    "\"",
                    "\"",
                    25,
                ),
                GrammarRule::begin_end_rule(
                    "string_single",
                    "string.quoted",
                    "'",
                    "'",
                    25,
                ),
            ],
            priority: 25,
        }]
    }
}
