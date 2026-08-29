use std::fmt;
use std::iter::Peekable;

use handl_plugin_lexer::primitives;
use handl_plugin_lexer::{
    CommentFactory, CommentTokenKind, DefaultLexError, EofFactory, KindFactory, IdentFactory,
    KeywordFactory, Lexer, LexerConfig, NumberFactory, OperatorFactory, Span, StringFactory,
    Token,
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
use handl_plugin_grammar_apply::ApplyToken;
use handl_plugin_grammar_array::ArrayToken;
use handl_plugin_grammar_assign::AssignToken;
use handl_plugin_grammar_call::CallToken;
use handl_plugin_grammar_handle::HandleToken;
use handl_plugin_grammar_index::IndexToken;
use handl_plugin_grammar_match::MatchToken;
use handl_plugin_grammar_object::ObjectToken;
use handl_plugin_grammar_pipe::PipeToken;
use handl_plugin_grammar_primary_identifier::IdentToken;
use handl_plugin_grammar_primary_value::ValueToken;
use handl_plugin_grammar_stmt_export::ExportStmtToken;
use handl_plugin_grammar_stmt_import::ImportStmtToken;
use handl_plugin_grammar_stmt_module::ModuleStmtToken;
use handl_plugin_grammar_template::TemplateToken;
use handl_plugin_grammar_function_literal::ArrowToken;
use handl_plugin_grammar_type_annot::TypeAnnotToken;

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

pub use handl_plugin_lexer_template::TemplateSegment as CanonicalTemplateSegment;

/// Canonical keyword forms used by the kit token kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanonicalKeyword {
    Match,
    Handle,
    As,
    TypeOf,
    Not,
    Import,
    Export,
    Module,
}

/// Canonical delimiter forms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanonicalDelim {
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

/// Canonical operator forms.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalOp {
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

handl_plugin_lexer::compose_token_kind! {
    /// Kit-level unified token kind for the canonical dialect.
    pub enum CanonicalKind {
        factory: CanonicalFactory;

        Eof,
        Number(f64),
        String(String),
        Template(Vec<TemplateSegment>),
        Identifier(String),
        Bool(bool),
        Null,
        Comment(String),
        DocComment(String),
        Delimiter(CanonicalDelim),
        Operator(CanonicalOp),
        Keyword(CanonicalKeyword),
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
        ParenOpen => Delimiter(CanonicalDelim::ParenOpen),
        ParenClose => Delimiter(CanonicalDelim::ParenClose),
        BracketOpen => Delimiter(CanonicalDelim::BracketOpen),
        BracketClose => Delimiter(CanonicalDelim::BracketClose),
        BraceOpen => Delimiter(CanonicalDelim::BraceOpen),
        BraceClose => Delimiter(CanonicalDelim::BraceClose),
        Comma => Delimiter(CanonicalDelim::Comma),
        Colon => Delimiter(CanonicalDelim::Colon),
        Dot => Delimiter(CanonicalDelim::Dot),
        Semicolon => Delimiter(CanonicalDelim::Semicolon),
        Eof => Eof,
    }
    OperatorKind {
        Pipe => Operator(CanonicalOp::Pipe),
        Arrow => Operator(CanonicalOp::Arrow),
        Equal => Operator(CanonicalOp::Equal),
        Plus => Operator(CanonicalOp::Plus),
        Minus => Operator(CanonicalOp::Minus),
        Star => Operator(CanonicalOp::Star),
        Slash => Operator(CanonicalOp::Slash),
        LessThan => Operator(CanonicalOp::LessThan),
        GreaterThan => Operator(CanonicalOp::GreaterThan),
        Eof => Eof,
    }
    AsKind {
        As => Keyword(CanonicalKeyword::As),
        Eof => Eof,
    }
    HandleKind {
        Handle => Keyword(CanonicalKeyword::Handle),
        Eof => Eof,
    }
}

const KEYWORD_DISPLAY: &[(CanonicalKeyword, &str)] = &[
    (CanonicalKeyword::Match, "match"),
    (CanonicalKeyword::Handle, "handle"),
    (CanonicalKeyword::As, "as"),
    (CanonicalKeyword::TypeOf, "typeof"),
    (CanonicalKeyword::Not, "not"),
    (CanonicalKeyword::Import, "import"),
    (CanonicalKeyword::Export, "export"),
    (CanonicalKeyword::Module, "module"),
];

const DELIM_DISPLAY: &[(CanonicalDelim, &str)] = &[
    (CanonicalDelim::ParenOpen, "("),
    (CanonicalDelim::ParenClose, ")"),
    (CanonicalDelim::BracketOpen, "["),
    (CanonicalDelim::BracketClose, "]"),
    (CanonicalDelim::BraceOpen, "{{"),
    (CanonicalDelim::BraceClose, "}}"),
    (CanonicalDelim::Comma, ","),
    (CanonicalDelim::Colon, ":"),
    (CanonicalDelim::Dot, "."),
    (CanonicalDelim::Semicolon, ";"),
];

impl fmt::Display for CanonicalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanonicalKind::Number(n) => write!(f, "number({n})"),
            CanonicalKind::String(s) => write!(f, "string({s:?})"),
            CanonicalKind::Template(segments) => {
                write!(f, "template(")?;
                for seg in segments {
                    match seg {
                        TemplateSegment::Text(t) => write!(f, "{t:?}")?,
                        TemplateSegment::Expr(e) => write!(f, "${{{e}}}")?,
                    }
                }
                write!(f, ")")
            }
            CanonicalKind::Identifier(s) => write!(f, "identifier({s})"),
            CanonicalKind::Bool(b) => write!(f, "bool({b})"),
            CanonicalKind::Null => write!(f, "null"),
            CanonicalKind::Comment(s) => write!(f, "comment({s:?})"),
            CanonicalKind::DocComment(s) => write!(f, "doc-comment({s:?})"),
            CanonicalKind::Delimiter(d) => {
                let text = DELIM_DISPLAY
                    .iter()
                    .find(|(delim, _)| *delim == *d)
                    .map(|(_, t)| *t)
                    .unwrap_or("");
                write!(f, "{text}")
            }
            CanonicalKind::Operator(o) => match o {
                CanonicalOp::Pipe => write!(f, "|>"),
                CanonicalOp::Arrow => write!(f, "->"),
                CanonicalOp::Equal => write!(f, "="),
                CanonicalOp::Plus => write!(f, "+"),
                CanonicalOp::Minus => write!(f, "-"),
                CanonicalOp::Star => write!(f, "*"),
                CanonicalOp::Slash => write!(f, "/"),
                CanonicalOp::LessThan => write!(f, "<"),
                CanonicalOp::GreaterThan => write!(f, ">"),
                CanonicalOp::Other(s) => write!(f, "operator({s})"),
            },
            CanonicalKind::Keyword(k) => {
                let text = KEYWORD_DISPLAY
                    .iter()
                    .find(|(kw, _)| *kw == *k)
                    .map(|(_, t)| *t)
                    .unwrap_or("");
                write!(f, "{text}")
            }
            CanonicalKind::Eof => write!(f, "eof"),
        }
    }
}

/// The factory that turns raw values into [`CanonicalKind`] tokens.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonicalFactory;

impl CanonicalFactory {
    /// Returns all keyword text and their kinds, as the single source of truth.
    pub fn all_keywords() -> &'static [(&'static str, CanonicalKind)] {
        &[
            ("true", CanonicalKind::Bool(true)),
            ("false", CanonicalKind::Bool(false)),
            ("null", CanonicalKind::Null),
            ("match", CanonicalKind::Keyword(CanonicalKeyword::Match)),
            ("handle", CanonicalKind::Keyword(CanonicalKeyword::Handle)),
            ("as", CanonicalKind::Keyword(CanonicalKeyword::As)),
            ("typeof", CanonicalKind::Keyword(CanonicalKeyword::TypeOf)),
            ("not", CanonicalKind::Keyword(CanonicalKeyword::Not)),
            ("import", CanonicalKind::Keyword(CanonicalKeyword::Import)),
            ("export", CanonicalKind::Keyword(CanonicalKeyword::Export)),
            ("module", CanonicalKind::Keyword(CanonicalKeyword::Module)),
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

impl EofFactory<CanonicalKind> for CanonicalFactory {
    fn eof_kind(&self) -> CanonicalKind {
        CanonicalKind::Eof
    }
}

impl KindFactory<CanonicalKind> for CanonicalFactory {
    fn string_kind(&self, value: String) -> CanonicalKind {
        CanonicalKind::String(value)
    }

    fn number_kind(&self, value: f64) -> CanonicalKind {
        CanonicalKind::Number(value)
    }

    fn ident_kind(&self, text: String) -> CanonicalKind {
        CanonicalKind::Identifier(text)
    }

    fn keyword_kind(&self, text: &str) -> Option<CanonicalKind> {
        Self::all_keywords()
            .iter()
            .find(|(kw, _)| *kw == text)
            .map(|(_, kind)| kind.clone())
    }

    fn operator_kind(&self, text: &str) -> Option<CanonicalKind> {
        let op = match text {
            "|>" => CanonicalOp::Pipe,
            "->" => CanonicalOp::Arrow,
            "=" => CanonicalOp::Equal,
            "+" => CanonicalOp::Plus,
            "-" => CanonicalOp::Minus,
            "*" => CanonicalOp::Star,
            "/" => CanonicalOp::Slash,
            "<" => CanonicalOp::LessThan,
            ">" => CanonicalOp::GreaterThan,
            _ => CanonicalOp::Other(text.to_string()),
        };
        Some(CanonicalKind::Operator(op))
    }
}

impl StringFactory<CanonicalKind> for CanonicalFactory {
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

impl NumberFactory<CanonicalKind> for CanonicalFactory {
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

impl IdentFactory<CanonicalKind> for CanonicalFactory {
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

impl KeywordFactory<CanonicalKind> for CanonicalFactory {}

impl OperatorFactory<CanonicalKind> for CanonicalFactory {
    fn lex_operator(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Option<(CanonicalKind, Span)> {
        let (text, span) = primitives::lex_operator(chars, CANONICAL_OPERATOR_CHARS)?;
        self.operator_kind(&text).map(|kind| (kind, span))
    }
}

impl CommentFactory<CanonicalKind> for CanonicalFactory {
    fn comment_kind(&self, text: String) -> CanonicalKind {
        CanonicalKind::Comment(text)
    }

    fn doc_comment_kind(&self, text: String) -> CanonicalKind {
        CanonicalKind::DocComment(text)
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

impl CommentTokenKind for CanonicalKind {
    fn is_comment(&self) -> bool {
        matches!(
            self,
            CanonicalKind::Comment(_) | CanonicalKind::DocComment(_)
        )
    }
}

impl ValueToken for CanonicalKind {
    fn as_value(&self) -> Option<serde_json::Value> {
        match self {
            CanonicalKind::Number(n) => serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .or(Some(serde_json::Value::Null)),
            CanonicalKind::String(s) => Some(serde_json::Value::String(s.clone())),
            CanonicalKind::Bool(b) => Some(serde_json::Value::Bool(*b)),
            CanonicalKind::Null => Some(serde_json::Value::Null),
            _ => None,
        }
    }
}

impl IdentToken for CanonicalKind {
    fn as_ident(&self) -> Option<&str> {
        match self {
            CanonicalKind::Identifier(name) => Some(name.as_str()),
            _ => None,
        }
    }
}

impl SemicolonToken for CanonicalKind {
    fn is_semicolon(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Semicolon))
    }
}

impl CallToken for CanonicalKind {
    fn is_call_open(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::ParenOpen))
    }

    fn is_call_close(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::ParenClose))
    }

    fn is_comma(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Comma))
    }
}

impl IndexToken for CanonicalKind {
    fn is_bracket_open(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::BracketOpen))
    }

    fn is_bracket_close(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::BracketClose))
    }

    fn is_dot(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Dot))
    }

    fn ident_text(&self) -> Option<&str> {
        match self {
            CanonicalKind::Identifier(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

impl PipeToken for CanonicalKind {
    fn is_pipe(&self) -> bool {
        matches!(self, CanonicalKind::Operator(CanonicalOp::Pipe))
    }
}

impl ArrowToken for CanonicalKind {
    fn is_arrow(&self) -> bool {
        matches!(self, CanonicalKind::Operator(CanonicalOp::Arrow))
    }
}

impl ImportStmtToken for CanonicalKind {
    fn is_import_stmt(&self) -> bool {
        matches!(self, CanonicalKind::Keyword(CanonicalKeyword::Import))
    }

    fn is_import_stmt_semicolon(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Semicolon))
    }
}

impl ExportStmtToken for CanonicalKind {
    fn is_export_stmt(&self) -> bool {
        matches!(self, CanonicalKind::Keyword(CanonicalKeyword::Export))
    }

    fn is_export_stmt_semicolon(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Semicolon))
    }
}

impl ModuleStmtToken for CanonicalKind {
    fn is_module_stmt(&self) -> bool {
        matches!(self, CanonicalKind::Keyword(CanonicalKeyword::Module))
    }

    fn as_module_stmt_name(&self) -> Option<&str> {
        match self {
            CanonicalKind::Identifier(name) => Some(name.as_str()),
            _ => None,
        }
    }

    fn is_module_stmt_equal(&self) -> bool {
        matches!(self, CanonicalKind::Operator(CanonicalOp::Equal))
    }

    fn is_module_stmt_semicolon(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Semicolon))
    }
}

impl AssignToken for CanonicalKind {
    fn is_equal(&self) -> bool {
        matches!(self, CanonicalKind::Operator(CanonicalOp::Equal))
    }
}

impl MatchToken for CanonicalKind {
    fn is_match(&self) -> bool {
        matches!(self, CanonicalKind::Keyword(CanonicalKeyword::Match))
    }

    fn is_brace_open(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::BraceOpen))
    }

    fn is_brace_close(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::BraceClose))
    }

    fn is_comma(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Comma))
    }

    fn is_colon(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Colon))
    }
}

impl HandleToken for CanonicalKind {
    fn is_handle(&self) -> bool {
        matches!(self, CanonicalKind::Keyword(CanonicalKeyword::Handle))
    }
}

impl ArrayToken for CanonicalKind {
    fn is_bracket_open(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::BracketOpen))
    }

    fn is_bracket_close(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::BracketClose))
    }

    fn is_comma(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Comma))
    }
}

impl ObjectToken for CanonicalKind {
    fn is_brace_open(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::BraceOpen))
    }

    fn is_brace_close(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::BraceClose))
    }

    fn is_comma(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Comma))
    }

    fn is_colon(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Colon))
    }

    fn as_object_key(&self) -> Option<String> {
        match self {
            CanonicalKind::Identifier(name) => Some(name.clone()),
            CanonicalKind::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl TemplateTokenKind for CanonicalKind {
    fn template_kind(segments: Vec<TemplateSegment>) -> Self {
        CanonicalKind::Template(segments)
    }
}

impl TemplateToken for CanonicalKind {
    fn as_template_segments(&self) -> Option<&[TemplateSegment]> {
        match self {
            CanonicalKind::Template(segments) => Some(segments.as_slice()),
            _ => None,
        }
    }
}

impl ApplyToken for CanonicalKind {
    fn is_paren_open(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::ParenOpen))
    }

    fn is_paren_close(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::ParenClose))
    }

    fn is_comma(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Comma))
    }
}

impl TypeAnnotToken for CanonicalKind {
    fn is_colon(&self) -> bool {
        matches!(self, CanonicalKind::Delimiter(CanonicalDelim::Colon))
    }

    fn is_equal(&self) -> bool {
        matches!(self, CanonicalKind::Operator(CanonicalOp::Equal))
    }

    fn ident_text(&self) -> Option<&str> {
        match self {
            CanonicalKind::Identifier(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// The kit's concrete lexer.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonicalLexer;

impl CanonicalLexer {
    /// Returns the lexer configuration used by this lexer.
    ///
    /// This exposes the config so tooling (grammar generators, semantic token
    /// providers) can introspect keywords, delimiters, and rules without
    /// duplicating them.
    pub fn config() -> LexerConfig<CanonicalKind, DefaultLexError> {
        LexerConfig::new(CanonicalFactory)
            .with_delimiter('(', CanonicalKind::Delimiter(CanonicalDelim::ParenOpen))
            .with_delimiter(')', CanonicalKind::Delimiter(CanonicalDelim::ParenClose))
            .with_delimiter('[', CanonicalKind::Delimiter(CanonicalDelim::BracketOpen))
            .with_delimiter(']', CanonicalKind::Delimiter(CanonicalDelim::BracketClose))
            .with_delimiter('{', CanonicalKind::Delimiter(CanonicalDelim::BraceOpen))
            .with_delimiter('}', CanonicalKind::Delimiter(CanonicalDelim::BraceClose))
            .with_delimiter(',', CanonicalKind::Delimiter(CanonicalDelim::Comma))
            .with_delimiter(':', CanonicalKind::Delimiter(CanonicalDelim::Colon))
            .with_delimiter('.', CanonicalKind::Delimiter(CanonicalDelim::Dot))
            .with_delimiter(';', CanonicalKind::Delimiter(CanonicalDelim::Semicolon))
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

impl Lexer<CanonicalKind, DefaultLexError> for CanonicalLexer {
    fn lex(&self, input: &str) -> Result<Vec<Token<CanonicalKind>>, DefaultLexError> {
        let tokens = Self::config().lex(input)?;

        Ok(tokens
            .into_iter()
            .filter(|t| !t.kind.is_comment())
            .collect())
    }
}
