use handl_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical delimiter tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum DelimiterKind {
    /// `(`
    ParenOpen,
    /// `)`
    ParenClose,
    /// `[`
    BracketOpen,
    /// `]`
    BracketClose,
    /// `{`
    BraceOpen,
    /// `}`
    BraceClose,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// `;`
    Semicolon,
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for DelimiterKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DelimiterKind::ParenOpen => write!(f, "("),
            DelimiterKind::ParenClose => write!(f, ")"),
            DelimiterKind::BracketOpen => write!(f, "["),
            DelimiterKind::BracketClose => write!(f, "]"),
            DelimiterKind::BraceOpen => write!(f, "{{"),
            DelimiterKind::BraceClose => write!(f, "}}"),
            DelimiterKind::Comma => write!(f, ","),
            DelimiterKind::Colon => write!(f, ":"),
            DelimiterKind::Dot => write!(f, "."),
            DelimiterKind::Semicolon => write!(f, ";"),
            DelimiterKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for DelimiterKind {
    fn is_eof(&self) -> bool {
        matches!(self, DelimiterKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }
}

/// Builds a delimiter token at the given span.
pub fn delimiter_token(kind: DelimiterKind, span: Span) -> Token<DelimiterKind> {
    Token::new(kind, span)
}

/// Builds an end-of-input delimiter token at the given span.
pub fn eof_token(span: Span) -> Token<DelimiterKind> {
    Token::new(DelimiterKind::Eof, span)
}
