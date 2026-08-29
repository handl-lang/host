//! Lexical token vocabulary, factory, and lexer for the handl dialect.
//!
//! Defines [`LexKind`] and its supporting enums ([`LexKeyword`],
//! [`LexDelimiter`], [`LexOperator`]) by composing the narrow token-kind
//! building blocks from the `plugin-token-*` crates. Also provides
//! [`LexFactory`] (the lexer factory) and [`LexLexer`] (the concrete lexer).
//!
//! This crate has **no grammar-rule dependencies**. Each `plugin-grammar-*`
//! crate implements its own token-capability trait for [`LexKind`], keeping
//! the wiring local to the rule that needs it.

use std::fmt;
use std::iter::Peekable;

use handl_plugin_lexer::primitives;
use handl_plugin_lexer::{
    compose_token_kind, CommentFactory, CommentTokenKind, DefaultLexError, EofFactory,
    IdentFactory, KindFactory, KeywordFactory, Lexer, LexerConfig, NumberFactory, OperatorFactory,
    Span, StringFactory, Token,
};
use handl_plugin_lexer_block_comment::BlockCommentRule;
use handl_plugin_lexer_delimiter::DelimiterRule;
use handl_plugin_lexer_ident::IdentRule;
use handl_plugin_lexer_keyword::KeywordRule;
use handl_plugin_lexer_line_comment::LineCommentRule;
use handl_plugin_lexer_number::NumberRule as LexNumberRule;
use handl_plugin_lexer_operator::OperatorRule;
use handl_plugin_lexer_string::StringRule as LexStringRule;
use handl_plugin_lexer_template::TemplateRule as LexTemplateRule;
use handl_plugin_lexer_template::TemplateSegment;
use handl_plugin_lexer_template::TemplateTokenKind;
use handl_plugin_ext_parser::SemicolonToken;

use handl_plugin_token_bool::BoolKind;
use handl_plugin_token_comment::CommentKind;
use handl_plugin_token_delimiter::DelimiterKind;
use handl_plugin_token_doc_comment::{
    DocCommentKind, DocCommentRule, is_doc_block_comment_start, is_doc_line_comment_start,
    lex_doc_block_comment, lex_doc_line_comment,
};
use handl_plugin_token_identifier::IdentKind;
use handl_plugin_token_keyword_as::AsKind;
use handl_plugin_token_keyword_handle::HandleKind;
use handl_plugin_token_null::NullKind;
use handl_plugin_token_number::NumberKind;
use handl_plugin_token_operator::{CANONICAL_OPERATOR_CHARS, OperatorKind};
use handl_plugin_token_string::StringKind;

pub use handl_plugin_lexer_template::TemplateSegment as LexTemplateSegment;

// ---------------------------------------------------------------------------
// Sub-enums
// ---------------------------------------------------------------------------

/// Keyword forms used by the lex token kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LexKeyword {
    Match,
    Handle,
    As,
    TypeOf,
    Not,
    Import,
    Export,
    Module,
}

/// Delimiter forms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LexDelimiter {
    ParenOpen,
    ParenClose,
    BracketOpen,
    BracketClose,
    BraceOpen,
    BraceClose,
    Comma,
    Colon,
    Dot,
    Semicolon,
}

/// Operator forms.
#[derive(Debug, Clone, PartialEq)]
pub enum LexOperator {
    Pipe,
    Arrow,
    Equal,
    Plus,
    Minus,
    Star,
    Slash,
    LessThan,
    GreaterThan,
    Other(String),
}

// ---------------------------------------------------------------------------
// Composed token kind
// ---------------------------------------------------------------------------

compose_token_kind! {
    /// Unified token kind for the handl dialect.
    pub enum LexKind {
        factory: LexFactory;

        Eof,
        Number(f64),
        String(String),
        Template(Vec<TemplateSegment>),
        Identifier(String),
        Bool(bool),
        Null,
        Comment(String),
        DocComment(String),
        Delimiter(LexDelimiter),
        Operator(LexOperator),
        Keyword(LexKeyword),
    }

    NumberKind {
        Number(v) => Number(v),
        Eof => Eof,
    }
    StringKind {
        String(v) => String(v),
        Eof => Eof,
    }
    IdentKind {
        Ident(v) => Identifier(v),
        Eof => Eof,
    }
    BoolKind {
        Bool(v) => Bool(v),
        Eof => Eof,
    }
    NullKind {
        Null => Null,
        Eof => Eof,
    }
    CommentKind {
        Comment(v) => Comment(v),
        Eof => Eof,
    }
    DocCommentKind {
        DocComment(v) => DocComment(v),
        Eof => Eof,
    }
    DelimiterKind {
        ParenOpen => Delimiter(LexDelimiter::ParenOpen),
        ParenClose => Delimiter(LexDelimiter::ParenClose),
        BracketOpen => Delimiter(LexDelimiter::BracketOpen),
        BracketClose => Delimiter(LexDelimiter::BracketClose),
        BraceOpen => Delimiter(LexDelimiter::BraceOpen),
        BraceClose => Delimiter(LexDelimiter::BraceClose),
        Comma => Delimiter(LexDelimiter::Comma),
        Colon => Delimiter(LexDelimiter::Colon),
        Dot => Delimiter(LexDelimiter::Dot),
        Semicolon => Delimiter(LexDelimiter::Semicolon),
        Eof => Eof,
    }
    OperatorKind {
        Pipe => Operator(LexOperator::Pipe),
        Arrow => Operator(LexOperator::Arrow),
        Equal => Operator(LexOperator::Equal),
        Plus => Operator(LexOperator::Plus),
        Minus => Operator(LexOperator::Minus),
        Star => Operator(LexOperator::Star),
        Slash => Operator(LexOperator::Slash),
        LessThan => Operator(LexOperator::LessThan),
        GreaterThan => Operator(LexOperator::GreaterThan),
        Eof => Eof,
    }
    AsKind {
        As => Keyword(LexKeyword::As),
        Eof => Eof,
    }
    HandleKind {
        Handle => Keyword(LexKeyword::Handle),
        Eof => Eof,
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

const KEYWORD_DISPLAY: &[(LexKeyword, &str)] = &[
    (LexKeyword::Match, "match"),
    (LexKeyword::Handle, "handle"),
    (LexKeyword::As, "as"),
    (LexKeyword::TypeOf, "typeof"),
    (LexKeyword::Not, "not"),
    (LexKeyword::Import, "import"),
    (LexKeyword::Export, "export"),
    (LexKeyword::Module, "module"),
];

const DELIM_DISPLAY: &[(LexDelimiter, &str)] = &[
    (LexDelimiter::ParenOpen, "("),
    (LexDelimiter::ParenClose, ")"),
    (LexDelimiter::BracketOpen, "["),
    (LexDelimiter::BracketClose, "]"),
    (LexDelimiter::BraceOpen, "{{"),
    (LexDelimiter::BraceClose, "}}"),
    (LexDelimiter::Comma, ","),
    (LexDelimiter::Colon, ":"),
    (LexDelimiter::Dot, "."),
    (LexDelimiter::Semicolon, ";"),
];

impl fmt::Display for LexKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexKind::Number(n) => write!(f, "number({n})"),
            LexKind::String(s) => write!(f, "string({s:?})"),
            LexKind::Template(segments) => {
                write!(f, "template(")?;
                for seg in segments {
                    match seg {
                        TemplateSegment::Text(t) => write!(f, "{t:?}")?,
                        TemplateSegment::Expr(e) => write!(f, "${{{e}}}")?,
                    }
                }
                write!(f, ")")
            }
            LexKind::Identifier(s) => write!(f, "identifier({s})"),
            LexKind::Bool(b) => write!(f, "bool({b})"),
            LexKind::Null => write!(f, "null"),
            LexKind::Comment(s) => write!(f, "comment({s:?})"),
            LexKind::DocComment(s) => write!(f, "doc-comment({s:?})"),
            LexKind::Delimiter(d) => {
                let text = DELIM_DISPLAY
                    .iter()
                    .find(|(delim, _)| *delim == *d)
                    .map(|(_, t)| *t)
                    .unwrap_or("");
                write!(f, "{text}")
            }
            LexKind::Operator(o) => match o {
                LexOperator::Pipe => write!(f, "|>"),
                LexOperator::Arrow => write!(f, "->"),
                LexOperator::Equal => write!(f, "="),
                LexOperator::Plus => write!(f, "+"),
                LexOperator::Minus => write!(f, "-"),
                LexOperator::Star => write!(f, "*"),
                LexOperator::Slash => write!(f, "/"),
                LexOperator::LessThan => write!(f, "<"),
                LexOperator::GreaterThan => write!(f, ">"),
                LexOperator::Other(s) => write!(f, "operator({s})"),
            },
            LexKind::Keyword(k) => {
                let text = KEYWORD_DISPLAY
                    .iter()
                    .find(|(kw, _)| *kw == *k)
                    .map(|(_, t)| *t)
                    .unwrap_or("");
                write!(f, "{text}")
            }
            LexKind::Eof => write!(f, "eof"),
        }
    }
}

// ---------------------------------------------------------------------------
// Infrastructure trait impls (not grammar-specific)
// ---------------------------------------------------------------------------

impl CommentTokenKind for LexKind {
    fn is_comment(&self) -> bool {
        matches!(
            self,
            LexKind::Comment(_) | LexKind::DocComment(_)
        )
    }
}

impl TemplateTokenKind for LexKind {
    fn template_kind(segments: Vec<TemplateSegment>) -> Self {
        LexKind::Template(segments)
    }
}

impl SemicolonToken for LexKind {
    fn is_semicolon(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::Semicolon))
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// The factory that turns raw values into [`LexKind`] tokens.
#[derive(Debug, Default, Clone, Copy)]
pub struct LexFactory;

impl LexFactory {
    /// Returns all keyword text and their kinds, as the single source of truth.
    pub fn all_keywords() -> &'static [(&'static str, LexKind)] {
        &[
            ("true", LexKind::Bool(true)),
            ("false", LexKind::Bool(false)),
            ("null", LexKind::Null),
            ("match", LexKind::Keyword(LexKeyword::Match)),
            ("handle", LexKind::Keyword(LexKeyword::Handle)),
            ("as", LexKind::Keyword(LexKeyword::As)),
            ("typeof", LexKind::Keyword(LexKeyword::TypeOf)),
            ("not", LexKind::Keyword(LexKeyword::Not)),
            ("import", LexKind::Keyword(LexKeyword::Import)),
            ("export", LexKind::Keyword(LexKeyword::Export)),
            ("module", LexKind::Keyword(LexKeyword::Module)),
        ]
    }

    /// Returns all operator strings recognized by this factory.
    pub fn all_operators() -> Vec<String> {
        vec![
            "|>".to_string(),
            "->".to_string(),
            "=".to_string(),
            "+".to_string(),
            "-".to_string(),
            "*".to_string(),
            "/".to_string(),
            "<".to_string(),
            ">".to_string(),
        ]
    }
}

impl EofFactory<LexKind> for LexFactory {
    fn eof_kind(&self) -> LexKind {
        LexKind::Eof
    }
}

impl KindFactory<LexKind> for LexFactory {
    fn string_kind(&self, value: String) -> LexKind {
        LexKind::String(value)
    }

    fn number_kind(&self, value: f64) -> LexKind {
        LexKind::Number(value)
    }

    fn ident_kind(&self, text: String) -> LexKind {
        LexKind::Identifier(text)
    }

    fn keyword_kind(&self, text: &str) -> Option<LexKind> {
        Self::all_keywords()
            .iter()
            .find(|(kw, _)| *kw == text)
            .map(|(_, kind)| kind.clone())
    }

    fn operator_kind(&self, text: &str) -> Option<LexKind> {
        let op = match text {
            "|>" => LexOperator::Pipe,
            "->" => LexOperator::Arrow,
            "=" => LexOperator::Equal,
            "+" => LexOperator::Plus,
            "-" => LexOperator::Minus,
            "*" => LexOperator::Star,
            "/" => LexOperator::Slash,
            "<" => LexOperator::LessThan,
            ">" => LexOperator::GreaterThan,
            _ => LexOperator::Other(text.to_string()),
        };
        Some(LexKind::Operator(op))
    }
}

impl StringFactory<LexKind> for LexFactory {
    fn is_string_start(&self, ch: char) -> bool {
        ch == '"' || ch == '\''
    }

    fn lex_string(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError> {
        primitives::lex_string(chars)
    }
}

impl NumberFactory<LexKind> for LexFactory {
    fn is_number_start(&self, ch: char) -> bool {
        ch.is_ascii_digit() || ch == '-'
    }

    fn lex_number(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(f64, Span), DefaultLexError> {
        primitives::lex_number(chars)
    }

    fn number_regex(&self) -> &str {
        r"-?\d+(\.\d+)?([eE][+-]?\d+)?"
    }
}

impl IdentFactory<LexKind> for LexFactory {
    fn is_ident_start(&self, ch: char) -> bool {
        ch.is_alphabetic() || ch == '_'
    }

    fn is_ident_continue(&self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    fn lex_ident(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError> {
        primitives::lex_ident(chars, |c| self.is_ident_continue(c))
    }

    fn identifier_regex(&self) -> &str {
        r"[a-zA-Z_.][a-zA-Z0-9_.]*"
    }
}

impl KeywordFactory<LexKind> for LexFactory {}

impl OperatorFactory<LexKind> for LexFactory {
    fn lex_operator(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Option<(LexKind, Span)> {
        let (text, span) = primitives::lex_operator(chars, CANONICAL_OPERATOR_CHARS)?;
        self.operator_kind(&text).map(|kind| (kind, span))
    }
}

impl CommentFactory<LexKind> for LexFactory {
    fn comment_kind(&self, text: String) -> LexKind {
        LexKind::Comment(text)
    }

    fn doc_comment_kind(&self, text: String) -> LexKind {
        LexKind::DocComment(text)
    }

    fn line_comment_prefix(&self) -> Option<&str> {
        Some("//")
    }

    fn block_comment_delims(&self) -> Option<(&str, &str)> {
        Some(("/*", "*/"))
    }

    fn doc_line_comment_prefix(&self) -> Option<&str> {
        Some("///")
    }

    fn doc_block_comment_delims(&self) -> Option<(&str, &str)> {
        Some(("/**", "*/"))
    }

    fn is_line_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool {
        primitives::starts_with(chars, "//") && !primitives::starts_with(chars, "///")
    }

    fn lex_line_comment(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> (String, Span) {
        primitives::lex_line_comment(chars, "//")
    }

    fn is_block_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool {
        primitives::starts_with(chars, "/*") && !primitives::starts_with(chars, "/**")
    }

    fn lex_block_comment(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError> {
        primitives::lex_block_comment(chars, "/*", "*/")
    }

    fn is_doc_line_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool {
        is_doc_line_comment_start(chars)
    }

    fn lex_doc_line_comment(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> (String, Span) {
        lex_doc_line_comment(chars)
    }

    fn is_doc_block_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool {
        is_doc_block_comment_start(chars)
    }

    fn lex_doc_block_comment(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError> {
        lex_doc_block_comment(chars)
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

/// The concrete lexer for the handl dialect.
#[derive(Debug, Default, Clone, Copy)]
pub struct LexLexer;

impl LexLexer {
    /// Returns the lexer configuration used by this lexer.
    ///
    /// Exposed so tooling (grammar generators, semantic token providers) can
    /// introspect keywords, delimiters, and rules without duplicating them.
    pub fn config() -> LexerConfig<LexKind, DefaultLexError> {
        LexerConfig::new(LexFactory)
            .with_delimiter('(', LexKind::Delimiter(LexDelimiter::ParenOpen))
            .with_delimiter(')', LexKind::Delimiter(LexDelimiter::ParenClose))
            .with_delimiter('[', LexKind::Delimiter(LexDelimiter::BracketOpen))
            .with_delimiter(']', LexKind::Delimiter(LexDelimiter::BracketClose))
            .with_delimiter('{', LexKind::Delimiter(LexDelimiter::BraceOpen))
            .with_delimiter('}', LexKind::Delimiter(LexDelimiter::BraceClose))
            .with_delimiter(',', LexKind::Delimiter(LexDelimiter::Comma))
            .with_delimiter(':', LexKind::Delimiter(LexDelimiter::Colon))
            .with_delimiter('.', LexKind::Delimiter(LexDelimiter::Dot))
            .with_delimiter(';', LexKind::Delimiter(LexDelimiter::Semicolon))
            .with_rule(DocCommentRule)
            .with_rule(LineCommentRule)
            .with_rule(BlockCommentRule)
            .with_rule(LexStringRule)
            .with_rule(LexTemplateRule)
            .with_rule(LexNumberRule)
            .with_rule(KeywordRule)
            .with_rule(IdentRule)
            .with_rule(OperatorRule)
            .with_rule(DelimiterRule)
    }
}

impl Lexer<LexKind, DefaultLexError> for LexLexer {
    fn lex(&self, input: &str) -> Result<Vec<Token<LexKind>>, DefaultLexError> {
        let tokens = Self::config().lex(input)?;

        Ok(tokens
            .into_iter()
            .filter(|t| !t.kind.is_comment())
            .collect())
    }
}
