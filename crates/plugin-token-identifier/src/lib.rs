#![allow(non_snake_case)]
use handl_plugin_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical identifier tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum IdentKind {
    /// A identifier token.
    Ident(String),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for IdentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentKind::Ident(v) => write!(f, "identifier({:?})", v),
            IdentKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for IdentKind {
    fn is_eof(&self) -> bool {
        matches!(self, IdentKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

pub fn Ident_token(value: String, span: Span) -> Token<IdentKind> {
    Token::new(IdentKind::Ident(value), span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<IdentKind> for TokenFactory {
    fn eof_kind(&self) -> IdentKind {
        IdentKind::Eof
    }
}
