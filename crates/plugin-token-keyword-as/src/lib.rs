use handl_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical as keyword tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum AsKind {
    /// A as keyword token.
    As,
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for AsKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsKind::As => write!(f, "as keyword"),
            AsKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for AsKind {
    fn is_eof(&self) -> bool {
        matches!(self, AsKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

pub fn As_token(span: Span) -> Token<AsKind> {
    Token::new(AsKind::As, span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_lexer::EofFactory<AsKind> for TokenFactory {
    fn eof_kind(&self) -> AsKind {
        AsKind::Eof
    }
}
