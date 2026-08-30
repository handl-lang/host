#![allow(non_snake_case)]
use handl_plugin_grammar_textmate::{GrammarContributor, GrammarRule};
use handl_plugin_lexer::{Span, Token, TokenKind};
use std::fmt;

/// Canonical single-character operator set for the canonical language.
pub const CANONICAL_OPERATOR_CHARS: &str = "+-*/=<>&|%!^~?";

/// Token kind for canonical operator tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum OperatorKind {
    /// `|>`
    Pipe,
    /// `->`
    Arrow,
    /// `=`
    Equal,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `**`
    Pow,
    /// `/`
    Slash,
    /// `<`
    LessThan,
    /// `>`
    GreaterThan,
    /// `==`
    EqualEqual,
    /// `!=`
    NotEqual,
    /// `<=`
    LessEqual,
    /// `>=`
    GreaterEqual,
    /// `<=>`
    Spaceship,
    /// `>>>`
    Flow,
    /// `&&&`
    Fanout,
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for OperatorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperatorKind::Pipe => write!(f, "|>"),
            OperatorKind::Arrow => write!(f, "->"),
            OperatorKind::Equal => write!(f, "="),
            OperatorKind::Plus => write!(f, "+"),
            OperatorKind::Minus => write!(f, "-"),
            OperatorKind::Star => write!(f, "*"),
            OperatorKind::Pow => write!(f, "**"),
            OperatorKind::Slash => write!(f, "/"),
            OperatorKind::LessThan => write!(f, "<"),
            OperatorKind::GreaterThan => write!(f, ">"),
            OperatorKind::EqualEqual => write!(f, "=="),
            OperatorKind::NotEqual => write!(f, "!="),
            OperatorKind::LessEqual => write!(f, "<="),
            OperatorKind::GreaterEqual => write!(f, ">="),
            OperatorKind::Spaceship => write!(f, "<=>"),
            OperatorKind::Flow => write!(f, ">>>"),
            OperatorKind::Fanout => write!(f, "&&&"),
            OperatorKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for OperatorKind {
    fn is_eof(&self) -> bool {
        matches!(self, OperatorKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

/// Builds an operator token at the given span.
pub fn operator_token(kind: OperatorKind, span: Span) -> Token<OperatorKind> {
    Token::new(kind, span)
}

/// Builds an end-of-input operator token at the given span.
pub fn eof_token(span: Span) -> Token<OperatorKind> {
    Token::new(OperatorKind::Eof, span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<OperatorKind> for TokenFactory {
    fn eof_kind(&self) -> OperatorKind {
        OperatorKind::Eof
    }
}

fn regex_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$'
            | '/' => format!("\\{c}"),
            _ => c.to_string(),
        })
        .collect()
}

/// All operator strings recognized by this plugin, in priority order
/// (longest first so that multi-char operators match before single-char).
pub const OPERATOR_STRINGS: &[&str] = &[
    "|>", "->", "<=>", ">>>", "&&&", "==", "!=", "<=", ">=", "**", "=", "+", "-", "*", "/", "<", ">",
];

impl GrammarContributor for TokenFactory {
    fn grammar_rules(&self) -> Vec<GrammarRule> {
        let alt = OPERATOR_STRINGS
            .iter()
            .map(|s| regex_escape(s))
            .collect::<Vec<_>>()
            .join("|");
        vec![GrammarRule::match_rule(
            "operators",
            "keyword.operator",
            &format!("({alt})"),
            55,
        )]
    }
}
