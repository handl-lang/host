#![allow(non_snake_case)]
use handl_plugin_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical bool tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum BoolKind {
    /// A bool token.
    Bool(bool),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for BoolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoolKind::Bool(v) => write!(f, "bool({:?})", v),
            BoolKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for BoolKind {
    fn is_eof(&self) -> bool {
        matches!(self, BoolKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

pub fn Bool_token(value: bool, span: Span) -> Token<BoolKind> {
    Token::new(BoolKind::Bool(value), span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<BoolKind> for TokenFactory {
    fn eof_kind(&self) -> BoolKind {
        BoolKind::Eof
    }
}
