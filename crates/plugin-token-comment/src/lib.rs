#![allow(non_snake_case)]
use handl_plugin_grammar_textmate::{GrammarContributor, GrammarRule};
use handl_plugin_lexer::{Span, Token, TokenKind};
use std::fmt;

fn regex_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$'
            | '/' => format!("\\{c}"),
            _ => c.to_string(),
        })
        .collect()
}

/// Token kind for canonical comment tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum CommentKind {
    /// A comment token.
    Comment(String),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for CommentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommentKind::Comment(v) => write!(f, "comment({:?})", v),
            CommentKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for CommentKind {
    fn is_eof(&self) -> bool {
        matches!(self, CommentKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}
impl handl_plugin_lexer::CommentTokenKind for CommentKind {
    fn is_comment(&self) -> bool {
        matches!(self, CommentKind::Comment(_))
    }
}

pub fn Comment_token(value: String, span: Span) -> Token<CommentKind> {
    Token::new(CommentKind::Comment(value), span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<CommentKind> for TokenFactory {
    fn eof_kind(&self) -> CommentKind {
        CommentKind::Eof
    }
}

impl GrammarContributor for TokenFactory {
    fn grammar_rules(&self) -> Vec<GrammarRule> {
        vec![
            GrammarRule::match_rule(
                "comments",
                "comment.line",
                &format!("{}.*", regex_escape("//")),
                20,
            ),
            GrammarRule::begin_end_rule(
                "block_comment",
                "comment.block",
                &regex_escape("/*"),
                &regex_escape("*/"),
                21,
            ),
        ]
    }
}
