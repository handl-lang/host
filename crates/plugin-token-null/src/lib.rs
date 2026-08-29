#![allow(non_snake_case)]
use handl_plugin_grammar_textmate::{GrammarContributor, GrammarRule};
use handl_plugin_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical null tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum NullKind {
    /// A null token.
    Null,
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for NullKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NullKind::Null => write!(f, "null"),
            NullKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for NullKind {
    fn is_eof(&self) -> bool {
        matches!(self, NullKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

pub fn Null_token(span: Span) -> Token<NullKind> {
    Token::new(NullKind::Null, span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<NullKind> for TokenFactory {
    fn eof_kind(&self) -> NullKind {
        NullKind::Eof
    }
}

impl GrammarContributor for TokenFactory {
    fn grammar_rules(&self) -> Vec<GrammarRule> {
        vec![GrammarRule::match_rule(
            "nulls",
            "constant.language.null",
            r"\b(null)\b",
            41,
        )]
    }
}
