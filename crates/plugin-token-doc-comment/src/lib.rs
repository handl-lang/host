#![allow(non_snake_case)]
use handl_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical doc-comment tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum DocCommentKind {
    /// A doc comment token.
    DocComment(String),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for DocCommentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocCommentKind::DocComment(v) => write!(f, "doc-comment({:?})", v),
            DocCommentKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for DocCommentKind {
    fn is_eof(&self) -> bool {
        matches!(self, DocCommentKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

pub fn DocComment_token(value: String, span: Span) -> Token<DocCommentKind> {
    Token::new(DocCommentKind::DocComment(value), span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_lexer::EofFactory<DocCommentKind> for TokenFactory {
    fn eof_kind(&self) -> DocCommentKind {
        DocCommentKind::Eof
    }
}
