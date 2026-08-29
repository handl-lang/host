#![allow(non_snake_case)]
use handl_plugin_grammar_textmate::{GrammarContributor, GrammarRule};
use handl_plugin_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Token kind for canonical handle keyword tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleKind {
    /// A handle keyword token.
    Handle,
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for HandleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandleKind::Handle => write!(f, "handle keyword"),
            HandleKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for HandleKind {
    fn is_eof(&self) -> bool {
        matches!(self, HandleKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

pub fn Handle_token(span: Span) -> Token<HandleKind> {
    Token::new(HandleKind::Handle, span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<HandleKind> for TokenFactory {
    fn eof_kind(&self) -> HandleKind {
        HandleKind::Eof
    }
}

impl GrammarContributor for TokenFactory {
    fn grammar_rules(&self) -> Vec<GrammarRule> {
        vec![GrammarRule::match_rule(
            "keyword_handle",
            "keyword",
            r"\b(handle)\b",
            50,
        )]
    }
}
