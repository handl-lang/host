//! Canonical number-token plugin.
//!
//! Provides the `NumberKind` token kind and implements
//! `handl_lexer::TokenKind`, the base capability trait from the generic
//! lexer crate.

use handl_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical number literals.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberKind {
    /// A numeric literal.
    Number(f64),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for NumberKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumberKind::Number(n) => write!(f, "number({n})"),
            NumberKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for NumberKind {
    fn is_eof(&self) -> bool {
        matches!(self, NumberKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }
}

/// Builds a `Token<NumberKind>` for a numeric literal at the given span.
pub fn number_token(value: f64, span: Span) -> Token<NumberKind> {
    Token::new(NumberKind::Number(value), span)
}

/// Builds the end-of-input `Token<NumberKind>` at the end of source.
pub fn eof_token(span: Span) -> Token<NumberKind> {
    Token::new(NumberKind::Eof, span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_is_not_eof_or_error() {
        let k = NumberKind::Number(42.0);
        assert!(!k.is_eof());
        assert!(!k.is_error());
        assert_eq!(k.to_string(), "number(42)");
    }

    #[test]
    fn eof_is_eof() {
        let k = NumberKind::Eof;
        assert!(k.is_eof());
        assert!(!k.is_error());
        assert_eq!(k.to_string(), "eof");
    }

    #[test]
    fn token_constructors() {
        let t = number_token(3.14, Span::new(0, 4));
        assert_eq!(t.kind, NumberKind::Number(3.14));
        assert_eq!(t.span, Span::new(0, 4));
    }
}
