use handl_lexer::{Span, Token, TokenKind};
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
}

pub fn String_token(value: String, span: Span) -> Token<StringKind> {
    Token::new(StringKind::String(value), span)
}
