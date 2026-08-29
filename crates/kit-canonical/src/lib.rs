//! Canonical kit as a generic plugin host.
//!
//! `Kit` loads a [`KitManifest`], registers the core canonical plugins, and
//! composes a [`Lexer`], a parser, and a node factory from them. It no longer
//! depends on the concrete `handl-ast-canonical`, `handl-lexer-canonical`, or
//! `handl-parser-canonical` crates.

use std::collections::HashSet;
use std::fmt;
use std::iter::Peekable;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_plugin_ast::{Expr as ExprTrait, Node, NodeMetadata, Stmt as StmtTrait};
use handl_plugin_lexer::primitives;
use handl_plugin_lexer::{
    CommentFactory, CommentTokenKind, DefaultLexError, EofFactory, GrammarInfo,
    GrammarInfoProvider, IdentFactory, KitGrammarMetadata, KeywordFactory, KindFactory, Lexer,
    LexerConfig, NumberFactory, OperatorFactory, Span, StringFactory, Token,
};
use handl_plugin_lexer_block_comment::BlockCommentRule;
use handl_plugin_lexer_delimiter::DelimiterRule;
use handl_plugin_lexer_ident::IdentRule;
use handl_plugin_lexer_keyword::KeywordRule;
use handl_plugin_lexer_line_comment::LineCommentRule;
use handl_plugin_lexer_number::NumberRule as LexNumberRule;
use handl_plugin_lexer_operator::OperatorRule;
use handl_plugin_lexer_string::StringRule as LexStringRule;
use handl_plugin_lexer_template::{
    TemplateRule as LexTemplateRule, TemplateSegment, TemplateTokenKind,
};
pub use handl_plugin_lexer_template::TemplateSegment as CanonicalTemplateSegment;

use handl_platform::{
    AstKindDeclaration, AstKindRole, AstNode, BasicEvaluator, BasicTypechecker, Behavior,
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    Operation, OptionsPlugin, PlatformError, Plugin, PluginContext, PluginManifest, RuleAcceptor,
    Span as PlatformSpan,
};
use handl_plugin_diag_expression::ExpressionDiagnostics;
use handl_plugin_ext_build::BasicBuilder;
use handl_plugin_ext_compile::BasicCompiler;
use handl_plugin_ext_fmt::BasicFormatter;
use handl_plugin_ext_lexer::BasicLexer;
use handl_plugin_ext_lint::BasicLinter;
use handl_plugin_ext_parser::{BasicParser, Parser, ParserBehavior, ParserContext, SemicolonToken};
use serde_json::json;
use handl_plugin_fmt_stmt::StatementFormatter;
use handl_plugin_fmt_trivia::TriviaFormatter;

use handl_plugin_load::LoaderHostPlugin;
use handl_plugin_loader_native::NativeLoader;
use handl_plugin_loader_wasm::WasmLoader;

use handl_plugin_ast_apply::Apply;
use handl_plugin_ast_array::Array;
use handl_plugin_ast_assign::Assign;
use handl_plugin_ast_call::Call;
use handl_plugin_ast_handle::Handle;
use handl_plugin_ast_index::Index;
use handl_plugin_ast_let::Let;
use handl_plugin_ast_match::Match;
use handl_plugin_ast_object::Object;
use handl_plugin_ast_pipe::Pipe;
use handl_plugin_ast_var::Var;
use handl_plugin_eval_hello::HelloEvalPlugin;
use handl_plugin_eval_json::JsonEvalPlugin;
use handl_plugin_grammar_apply::{ApplyRule, ApplyToken};
use handl_plugin_grammar_array::{ArrayRule, ArrayToken};
use handl_plugin_grammar_assign::{AssignRule, AssignToken};
use handl_plugin_grammar_call::{CallRule, CallToken};
use handl_plugin_grammar_handle::{HandleRule, HandleToken};
use handl_plugin_grammar_index::{IndexRule, IndexToken};
use handl_plugin_grammar_match::{MatchRule, MatchToken};
use handl_plugin_grammar_object::{ObjectRule, ObjectToken};
use handl_plugin_grammar_pipe::{PipeRule, PipeToken};
use handl_plugin_grammar_primary_identifier::{IdentToken, IdentifierRule};
use handl_plugin_grammar_primary_value::{ValueRule, ValueToken};
use handl_plugin_grammar_stmt_export::{ExportStmtRule, ExportStmtToken};
use handl_plugin_grammar_stmt_import::{ImportStmtRule, ImportStmtToken};
use handl_plugin_grammar_stmt_module::{ModuleStmtRule, ModuleStmtToken};
use handl_plugin_grammar_template::{TemplateRule, TemplateToken};
use handl_plugin_runtime::RuntimeHost;
use handl_plugin_runtime_apply::ApplyPlugin;
use handl_plugin_runtime_arithmetic::ArithmeticPlugin;
use handl_plugin_runtime_array::ArrayPlugin;
use handl_plugin_runtime_boolean::BooleanPlugin;
use handl_plugin_runtime_object::ObjectPlugin;
use handl_plugin_target_native::NativeTarget;
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

pub mod diagnostics;
pub use diagnostics::{Diagnostic, DiagnosticLevel, Diagnostics};

pub mod statement;
pub use statement::{ModuleOutput, Stmt, StmtKind, expr_to_pattern};

pub mod kit_manifest;
pub use kit_manifest::{KitManifest, canonical_manifest};

/// A generic metadata value for kit nodes.
pub type KitMetadata = ();

static UNIT: KitMetadata = ();

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
        ch.is_alphabetic() || ch == '_' || ch == '.'
    }

    fn is_ident_continue(&self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_' || ch == '.'
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

// Token capability implementations for the grammar-rule plugins.
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
}

impl PipeToken for CanonicalKind {
    fn is_pipe(&self) -> bool {
        matches!(self, CanonicalKind::Operator(CanonicalOp::Pipe))
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

/// A runtime value carried by a kit node.
#[derive(Debug, Clone, PartialEq)]
pub enum KitValue {
    /// A numeric value.
    Number(f64),
    /// A string literal.
    String(String),
    /// A boolean value.
    Bool(bool),
    /// The `null` value.
    Null,
}

/// An `import` statement: `import source`.
#[derive(Debug, Clone)]
pub struct Import<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    /// The imported source expression.
    pub source: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Import<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new `Import` node with default metadata.
    pub fn new(source: E) -> Self {
        Self {
            source,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Import<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.metadata == other.metadata
    }
}

impl<E, M> Node for Import<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    type Value = E::Value;
    type Metadata = M;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn with_metadata(self, metadata: Self::Metadata) -> Self
    where
        Self: Sized,
    {
        Self {
            source: self.source,
            metadata,
        }
    }
}

/// An `export` statement: `export value`.
#[derive(Debug, Clone)]
pub struct Export<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    /// The exported value expression.
    pub value: E,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Export<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new `Export` node with default metadata.
    pub fn new(value: E) -> Self {
        Self {
            value,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Export<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.metadata == other.metadata
    }
}

impl<E, M> Node for Export<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    type Value = E::Value;
    type Metadata = M;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn with_metadata(self, metadata: Self::Metadata) -> Self
    where
        Self: Sized,
    {
        Self {
            value: self.value,
            metadata,
        }
    }
}

/// A `module` declaration: `module name` or `module name = value`.
#[derive(Debug, Clone)]
pub struct Module<E, M = ()>
where
    E: Node,
    M: NodeMetadata,
{
    /// The module name.
    pub name: String,
    /// An optional module body expression.
    pub value: Option<E>,
    /// Node metadata.
    pub metadata: M,
}

impl<E, M> Module<E, M>
where
    E: Node,
    M: NodeMetadata,
{
    /// Creates a new `Module` node with default metadata.
    pub fn new(name: String, value: Option<E>) -> Self {
        Self {
            name,
            value,
            metadata: M::default(),
        }
    }
}

impl<E, M> PartialEq for Module<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value && self.metadata == other.metadata
    }
}

impl<E, M> Node for Module<E, M>
where
    E: Node,
    M: NodeMetadata + PartialEq,
{
    type Value = E::Value;
    type Metadata = M;

    fn metadata(&self) -> &Self::Metadata {
        &self.metadata
    }

    fn with_metadata(self, metadata: Self::Metadata) -> Self
    where
        Self: Sized,
    {
        Self {
            name: self.name,
            value: self.value,
            metadata,
        }
    }
}

/// Generic AST node produced by the canonical kit.
#[derive(Debug, Clone, PartialEq)]
pub enum KitNode {
    /// A literal value.
    Literal(KitValue),
    /// A variable reference.
    Var(Var<KitNode, KitMetadata>),
    /// A named function call.
    Call(Call<KitNode, KitMetadata>),
    /// A function-value application.
    Apply(Box<Apply<KitNode, KitMetadata>>),
    /// A postfix index.
    Index(Box<Index<KitNode, KitMetadata>>),
    /// A pipe expression.
    Pipe(Box<Pipe<KitNode, KitMetadata>>),
    /// An array literal.
    Array(Array<KitNode, KitMetadata>),
    /// An object literal.
    Object(Object<KitNode, KitMetadata>),
    /// A `let` binding.
    Let(Box<Let<KitNode, KitMetadata>>),
    /// An assignment expression.
    Assign(Box<Assign<KitNode, KitMetadata>>),
    /// A `match` expression.
    Match(Box<Match<KitNode, KitMetadata>>),
    /// A `handle` expression.
    Handle(Box<Handle<KitNode, KitMetadata>>),
    /// A statement block.
    Block(Array<KitNode, KitMetadata>),
    /// A `const` binding (reuses the `Let` shape: name + value).
    Const(Box<Let<KitNode, KitMetadata>>),
    /// An `import` statement.
    Import(Box<Import<KitNode, KitMetadata>>),
    /// An `export` statement.
    Export(Box<Export<KitNode, KitMetadata>>),
    /// A `module` declaration.
    Module(Box<Module<KitNode, KitMetadata>>),
}

impl Node for KitNode {
    type Value = KitValue;
    type Metadata = KitMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &UNIT
    }

    fn with_metadata(self, _metadata: Self::Metadata) -> Self
    where
        Self: Sized,
    {
        self
    }
}

impl ExprTrait for KitNode {
    fn value(value: Self::Value) -> Self
    where
        Self: Sized,
    {
        KitNode::Literal(value)
    }

    fn call<S>(name: S, args: Vec<Self>) -> Self
    where
        Self: Sized,
        S: Into<String>,
    {
        KitNode::Call(Call::new(name, args))
    }

    fn apply(target: Self, args: Vec<Self>) -> Self
    where
        Self: Sized,
    {
        if let KitNode::Var(v) = &target {
            KitNode::Call(Call::new(&v.name, args))
        } else {
            KitNode::Apply(Box::new(Apply::new(target, args)))
        }
    }

    fn array(elements: Vec<Self>) -> Self
    where
        Self: Sized,
    {
        KitNode::Array(Array::new(elements))
    }

    fn object(entries: Vec<(String, Self)>) -> Self
    where
        Self: Sized,
    {
        KitNode::Object(Object::new(entries))
    }

    fn index(target: Self, index: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Index(Box::new(Index::new(target, index)))
    }

    fn pipe(left: Self, right: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Pipe(Box::new(Pipe::new(left, right)))
    }

    fn assign(target: Self, value: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Assign(Box::new(Assign::new(target, value)))
    }

    fn match_(subject: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Match(Box::new(Match::new(subject)))
    }

    fn handle(expr: Self) -> Self
    where
        Self: Sized,
    {
        KitNode::Handle(Box::new(Handle::new(expr)))
    }
}

impl StmtTrait for KitNode {
    type Expr = KitNode;

    fn expression(expr: Self::Expr) -> Self
    where
        Self: Sized,
    {
        expr
    }

    fn let_<S>(name: S, value: Self::Expr) -> Self
    where
        Self: Sized,
        S: Into<String>,
    {
        KitNode::Let(Box::new(Let::new(name.into(), value)))
    }

    fn block(stmts: Vec<Self>) -> Self
    where
        Self: Sized,
    {
        KitNode::Block(Array::new(stmts))
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

/// Converts a generic `AstNode` produced by the parser into the kit's `KitNode`.
fn ast_to_kitnode(ast: &AstNode) -> Result<KitNode, PlatformError> {
    AST_CONVERTERS
        .iter()
        .find(|(kind, _)| *kind == ast.kind)
        .ok_or_else(|| {
            PlatformError::Plugin(format!("unsupported AST node kind '{}'", ast.kind))
        })
        .and_then(|(_, f)| f(ast))
}

type AstConverter = fn(&AstNode) -> Result<KitNode, PlatformError>;

const AST_CONVERTERS: &[(&str, AstConverter)] = &[
    ("value", convert_value),
    ("var", convert_var),
    ("call", convert_call),
    ("index", convert_index),
    ("pipe", convert_pipe),
    ("let", convert_let),
    ("assign", convert_assign),
    ("array", convert_array),
    ("object", convert_object),
    ("match", convert_match),
    ("handle", convert_handle),
    ("stmt.let", convert_stmt_let),
    ("stmt.const", convert_stmt_const),
    ("stmt.import", convert_stmt_import),
    ("stmt.export", convert_stmt_export),
    ("stmt.module", convert_stmt_module),
];

fn convert_value(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let value = ast.payload.get("value").ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in AST payload".into())
    })?;

    let kit_value = if value.is_null() {
        KitValue::Null
    } else if let Some(n) = value.as_f64() {
        KitValue::Number(n)
    } else if let Some(s) = value.as_str() {
        KitValue::String(s.to_string())
    } else if let Some(b) = value.as_bool() {
        KitValue::Bool(b)
    } else {
        return Err(PlatformError::Plugin(format!(
            "unsupported primary value: {value}"
        )));
    };

    Ok(KitNode::Literal(kit_value))
}

fn convert_var(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in var AST payload".into())
        })?;

    Ok(KitNode::Var(Var::new(name.into())))
}

fn convert_call(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let callee = ast.payload.get("callee").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'callee' field in call AST payload".into())
    })?;
    let callee_ast: AstNode = serde_json::from_value(callee)
        .map_err(|e| PlatformError::Plugin(format!("invalid callee in call AST: {e}")))?;
    let callee = ast_to_kitnode(&callee_ast)?;

    let args = ast
        .payload
        .get("args")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'args' field in call AST payload".into())
        })?;
    let mut call_args = Vec::with_capacity(args.len());
    for arg in args {
        let arg_ast: AstNode = serde_json::from_value(arg.clone())
            .map_err(|e| PlatformError::Plugin(format!("invalid arg in call AST: {e}")))?;
        call_args.push(ast_to_kitnode(&arg_ast)?);
    }

    match callee {
        KitNode::Var(v) => Ok(KitNode::Call(Call::new(&v.name, call_args))),
        _ => Ok(KitNode::Apply(Box::new(Apply::new(callee, call_args)))),
    }
}

fn convert_index(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let target = ast.payload.get("target").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'target' field in index AST payload".into())
    })?;
    let target_ast: AstNode = serde_json::from_value(target)
        .map_err(|e| PlatformError::Plugin(format!("invalid target in index AST: {e}")))?;
    let target = ast_to_kitnode(&target_ast)?;

    let index = ast.payload.get("index").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'index' field in index AST payload".into())
    })?;
    let index_ast: AstNode = serde_json::from_value(index)
        .map_err(|e| PlatformError::Plugin(format!("invalid index in index AST: {e}")))?;
    let index = ast_to_kitnode(&index_ast)?;

    Ok(KitNode::Index(Box::new(Index::new(target, index))))
}

fn convert_pipe(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let left = ast.payload.get("left").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'left' field in pipe AST payload".into())
    })?;
    let left_ast: AstNode = serde_json::from_value(left)
        .map_err(|e| PlatformError::Plugin(format!("invalid left in pipe AST: {e}")))?;
    let left = ast_to_kitnode(&left_ast)?;

    let right = ast.payload.get("right").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'right' field in pipe AST payload".into())
    })?;
    let right_ast: AstNode = serde_json::from_value(right)
        .map_err(|e| PlatformError::Plugin(format!("invalid right in pipe AST: {e}")))?;
    let right = ast_to_kitnode(&right_ast)?;

    Ok(KitNode::Pipe(Box::new(Pipe::new(left, right))))
}

fn convert_let(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in let AST payload".into())
        })?;

    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in let AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value)
        .map_err(|e| PlatformError::Plugin(format!("invalid value in let AST: {e}")))?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Let(Box::new(Let::new(name.into(), value))))
}

fn convert_assign(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let target = ast.payload.get("target").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'target' field in assign AST payload".into())
    })?;
    let target_ast: AstNode = serde_json::from_value(target)
        .map_err(|e| PlatformError::Plugin(format!("invalid target in assign AST: {e}")))?;
    let target = ast_to_kitnode(&target_ast)?;

    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in assign AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value)
        .map_err(|e| PlatformError::Plugin(format!("invalid value in assign AST: {e}")))?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Assign(Box::new(Assign::new(target, value))))
}

fn convert_array(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let elements = ast
        .payload
        .get("elements")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'elements' field in array AST payload".into())
        })?;
    let mut elems = Vec::with_capacity(elements.len());
    for elem in elements {
        let elem_ast: AstNode = serde_json::from_value(elem.clone()).map_err(|e| {
            PlatformError::Plugin(format!("invalid element in array AST: {e}"))
        })?;
        elems.push(ast_to_kitnode(&elem_ast)?);
    }
    Ok(KitNode::Array(Array::new(elems)))
}

fn convert_object(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let entries = ast
        .payload
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'entries' field in object AST payload".into())
        })?;
    let mut pairs = Vec::with_capacity(entries.len());
    for entry in entries {
        let key = entry
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PlatformError::Plugin("missing 'key' in object entry".into()))?;
        let value_val = entry.get("value").cloned().ok_or_else(|| {
            PlatformError::Plugin("missing 'value' in object entry".into())
        })?;
        let value_ast: AstNode = serde_json::from_value(value_val).map_err(|e| {
            PlatformError::Plugin(format!("invalid value in object entry: {e}"))
        })?;
        pairs.push((key.to_string(), ast_to_kitnode(&value_ast)?));
    }
    Ok(KitNode::Object(Object::new(pairs)))
}

fn convert_match(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let subject = ast.payload.get("subject").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'subject' field in match AST payload".into())
    })?;
    let subject_ast: AstNode = serde_json::from_value(subject)
        .map_err(|e| PlatformError::Plugin(format!("invalid subject in match AST: {e}")))?;
    let subject = ast_to_kitnode(&subject_ast)?;
    Ok(KitNode::Match(Box::new(Match::new(subject))))
}

fn convert_handle(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let expr = ast.payload.get("expr").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'expr' field in handle AST payload".into())
    })?;
    let expr_ast: AstNode = serde_json::from_value(expr)
        .map_err(|e| PlatformError::Plugin(format!("invalid expr in handle AST: {e}")))?;
    let expr = ast_to_kitnode(&expr_ast)?;
    Ok(KitNode::Handle(Box::new(Handle::new(expr))))
}

fn convert_stmt_let(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in stmt.let AST payload".into())
        })?;

    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in stmt.let AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value)
        .map_err(|e| PlatformError::Plugin(format!("invalid value in stmt.let AST: {e}")))?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Let(Box::new(Let::new(name.into(), value))))
}

fn convert_stmt_const(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in stmt.const AST payload".into())
        })?;

    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in stmt.const AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value).map_err(|e| {
        PlatformError::Plugin(format!("invalid value in stmt.const AST: {e}"))
    })?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Const(Box::new(Let::new(name.into(), value))))
}

fn convert_stmt_import(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let source = ast.payload.get("source").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'source' field in stmt.import AST payload".into())
    })?;
    let source_ast: AstNode = serde_json::from_value(source).map_err(|e| {
        PlatformError::Plugin(format!("invalid source in stmt.import AST: {e}"))
    })?;
    let source = ast_to_kitnode(&source_ast)?;

    Ok(KitNode::Import(Box::new(Import::new(source))))
}

fn convert_stmt_export(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let value = ast.payload.get("value").cloned().ok_or_else(|| {
        PlatformError::Plugin("missing 'value' field in stmt.export AST payload".into())
    })?;
    let value_ast: AstNode = serde_json::from_value(value).map_err(|e| {
        PlatformError::Plugin(format!("invalid value in stmt.export AST: {e}"))
    })?;
    let value = ast_to_kitnode(&value_ast)?;

    Ok(KitNode::Export(Box::new(Export::new(value))))
}

fn convert_stmt_module(ast: &AstNode) -> Result<KitNode, PlatformError> {
    let name = ast
        .payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            PlatformError::Plugin("missing 'name' field in stmt.module AST payload".into())
        })?;

    let value = ast.payload.get("value").cloned();
    let value = match value {
        Some(v) => {
            let value_ast: AstNode = serde_json::from_value(v).map_err(|e| {
                PlatformError::Plugin(format!("invalid value in stmt.module AST: {e}"))
            })?;
            Some(ast_to_kitnode(&value_ast)?)
        }
        None => None,
    };

    Ok(KitNode::Module(Box::new(Module::new(name.into(), value))))
}

/// Like [`ast_to_kitnode`] but with access to the [`Kit`] for recursive
/// parsing of template interpolation expressions.
fn ast_to_kitnode_with_kit(ast: &AstNode, kit: &Kit) -> Result<KitNode, PlatformError> {
    match ast.kind.as_str() {
        "template" => {
            let segments = ast
                .payload
                .get("segments")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    PlatformError::Plugin("missing 'segments' field in template AST payload".into())
                })?;

            let mut parts: Vec<KitNode> = Vec::new();
            for seg in segments {
                let seg_type = seg.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
                    PlatformError::Plugin("missing 'type' in template segment".into())
                })?;

                match seg_type {
                    "text" => {
                        let text = seg.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
                            PlatformError::Plugin("missing 'value' in text segment".into())
                        })?;
                        parts.push(KitNode::Literal(KitValue::String(text.to_string())));
                    }
                    "expr" => {
                        let raw = seg.get("raw").and_then(|v| v.as_str()).ok_or_else(|| {
                            PlatformError::Plugin("missing 'raw' in expr segment".into())
                        })?;
                        let expr_node = kit.parse(raw)?;
                        parts.push(expr_node);
                    }
                    _ => {
                        return Err(PlatformError::Plugin(format!(
                            "unknown template segment type '{}'",
                            seg_type
                        )));
                    }
                }
            }

            if parts.is_empty() {
                Ok(KitNode::Literal(KitValue::String(String::new())))
            } else if parts.len() == 1 {
                Ok(parts.into_iter().next().unwrap())
            } else {
                let mut iter = parts.into_iter().rev();
                let mut acc = iter.next().unwrap();
                for part in iter {
                    acc = KitNode::Call(Call::new("str.concat", vec![part, acc]));
                }
                Ok(acc)
            }
        }
        _ => ast_to_kitnode(ast),
    }
}

/// Grammar rule for typed assignments: `name: type = value`.
///
/// This is an infix rule triggered by the `:` delimiter. It parses the
/// type name (an identifier), expects `=`, then parses the value
/// expression. Produces a `stmt.let` AST node with an optional `type` field.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypeAnnotRule;

impl Behavior<Parser<CanonicalKind>> for TypeAnnotRule {
    fn kind(&self) -> &str {
        "type-annot"
    }

    fn apply(&self, _op: &mut <Parser<CanonicalKind> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl ParserBehavior<CanonicalKind> for TypeAnnotRule {
    fn matches_prefix(&self, _kind: &CanonicalKind) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<CanonicalKind>,
        _ctx: &mut ParserContext<'_, CanonicalKind>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("TypeAnnotRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &CanonicalKind) -> Option<(u32, u32)> {
        if matches!(kind, CanonicalKind::Delimiter(CanonicalDelim::Colon)) {
            Some((15, 16))
        } else {
            None
        }
    }

    fn infix(
        &self,
        left: AstNode,
        _token: Token<CanonicalKind>,
        ctx: &mut ParserContext<'_, CanonicalKind>,
        _rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let name = left
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PlatformError::Plugin(format!("expected identifier before ':', got {:?}", left.kind))
            })?;

        let type_tok = ctx.advance()?;
        let type_name = match &type_tok.kind {
            CanonicalKind::Identifier(s) => s.clone(),
            _ => return Err(PlatformError::Plugin(format!(
                "expected type name after ':', got {}",
                type_tok.kind
            ))),
        };

        let eq = ctx.advance()?;
        if !matches!(eq.kind, CanonicalKind::Operator(CanonicalOp::Equal)) {
            return Err(PlatformError::Plugin(format!(
                "expected '=' after type annotation, got {}",
                eq.kind
            )));
        }

        let value = ctx.expression(0)?;

        let start = left.span.map_or(0, |s| s.start);
        let end = value.span.map_or(start, |s| s.end);

        Ok(AstNode {
            kind: "stmt.let".into(),
            span: Some(PlatformSpan { start, end }),
            payload: json!({
                "name": name,
                "type": type_name,
                "value": &value,
            }),
        })
    }
}

/// The set of grammar-rule plugin ids that the canonical kit knows how to
/// wire into a `BasicParser<CanonicalKind>`.
const GRAMMAR_RULE_PLUGIN_IDS: &[&str] = &[
    "plugin-grammar-primary-value",
    "plugin-grammar-primary-identifier",
    "plugin-grammar-call",
    "plugin-grammar-index",
    "plugin-grammar-pipe",
    "plugin-grammar-template",
    "plugin-grammar-apply",
    "plugin-grammar-assign",
    "plugin-grammar-type-annot",
    "plugin-grammar-array",
    "plugin-grammar-object",
    "plugin-grammar-match",
    "plugin-grammar-handle",
    "plugin-grammar-stmt-import",
    "plugin-grammar-stmt-export",
    "plugin-grammar-stmt-module",
];

/// Returns a freshly-constructed rule for the given grammar-rule plugin id, or
/// `None` if the id is not a recognized grammar rule.
fn rule_for_plugin_id(id: &str) -> Option<Box<dyn ParserBehavior<CanonicalKind>>> {
    let rule: Box<dyn ParserBehavior<CanonicalKind>> = match id {
        "plugin-grammar-call" => Box::new(CallRule),
        "plugin-grammar-apply" => Box::new(ApplyRule),
        "plugin-grammar-index" => Box::new(IndexRule),
        "plugin-grammar-pipe" => Box::new(PipeRule),
        "plugin-grammar-template" => Box::new(TemplateRule),
        "plugin-grammar-assign" => Box::new(AssignRule),
        "plugin-grammar-array" => Box::new(ArrayRule),
        "plugin-grammar-object" => Box::new(ObjectRule),
        "plugin-grammar-match" => Box::new(MatchRule),
        "plugin-grammar-handle" => Box::new(HandleRule),
        "plugin-grammar-primary-identifier" => Box::new(IdentifierRule),
        "plugin-grammar-primary-value" => Box::new(ValueRule),
        "plugin-grammar-type-annot" => Box::new(TypeAnnotRule),
        "plugin-grammar-stmt-import" => Box::new(ImportStmtRule),
        "plugin-grammar-stmt-export" => Box::new(ExportStmtRule),
        "plugin-grammar-stmt-module" => Box::new(ModuleStmtRule),
        _ => return None,
    };
    Some(rule)
}

/// Builds a parser populated with the grammar rules for the given plugin ids.
fn parser_for_plugins(ids: &[String]) -> Result<BasicParser<CanonicalKind>, PlatformError> {
    let mut parser = BasicParser::<CanonicalKind>::new();
    for id in ids {
        if let Some(rule) = rule_for_plugin_id(id) {
            parser.accept(rule)?;
        }
    }
    Ok(parser)
}

const AST_KIND_TABLE: &[(&str, &str, AstKindRole)] = &[
    ("plugin-grammar-primary-value", "value", AstKindRole::Produces),
    ("plugin-grammar-primary-identifier", "var", AstKindRole::Produces),
    ("plugin-grammar-call", "call", AstKindRole::Produces),
    ("plugin-grammar-apply", "apply", AstKindRole::Produces),
    ("plugin-grammar-index", "index", AstKindRole::Produces),
    ("plugin-grammar-pipe", "pipe", AstKindRole::Produces),
    ("plugin-grammar-assign", "assign", AstKindRole::Produces),
    ("plugin-grammar-array", "array", AstKindRole::Produces),
    ("plugin-grammar-object", "object", AstKindRole::Produces),
    ("plugin-grammar-match", "match", AstKindRole::Produces),
    ("plugin-grammar-handle", "handle", AstKindRole::Produces),
    ("plugin-grammar-type-annot", "stmt.let", AstKindRole::Produces),
    ("plugin-grammar-stmt-import", "stmt.import", AstKindRole::Produces),
    ("plugin-grammar-stmt-export", "stmt.export", AstKindRole::Produces),
    ("plugin-grammar-stmt-module", "stmt.module", AstKindRole::Produces),
    ("plugin-eval-hello", "hello.greet", AstKindRole::Consumes),
    ("plugin-eval-json", "json", AstKindRole::Consumes),
    ("plugin-typecheck-core", "typecheck", AstKindRole::Consumes),
    ("plugin-fmt-trivia", "fmt.trivia", AstKindRole::Consumes),
    ("plugin-fmt-stmt", "fmt.statements", AstKindRole::Consumes),
    ("plugin-diag-expression", "lint.expression", AstKindRole::Consumes),
];

/// Returns the AST kind declarations for a canonical plugin id: producers for
/// grammar rules and consumers for behavior-level plugins.
fn ast_kinds_for_plugin(id: &str) -> Vec<AstKindDeclaration> {
    AST_KIND_TABLE
        .iter()
        .find(|(pid, _, _)| *pid == id)
        .map(|(_, kind, role)| vec![AstKindDeclaration {
            kind: (*kind).into(),
            role: *role,
        }])
        .unwrap_or_default()
}

/// Synthesizes minimal `PluginManifest`s for the given plugin ids so the
/// resolver has something to select from.
fn manifests_for_plugins(ids: &[String]) -> Vec<PluginManifest> {
    ids.iter()
        .map(|id| PluginManifest {
            id: id.clone(),
            version: "0.1.0".to_string(),
            provides: vec![CapabilityId::from(id.as_str())],
            requires: Vec::new(),
            optional: Vec::new(),
            extends: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
            targets: Vec::new(),
            exclusive: Vec::new(),
            metadata: serde_json::Value::Null,
            ast_kinds: ast_kinds_for_plugin(id),
        })
        .collect()
}

/// Resolves a kit manifest's plugin list into a `ResolvedComposition`,
/// filtering to the grammar-rule plugins the canonical kit understands and
/// returning them in the order the parser expects (primary rules first).
fn resolve_grammar_rules(plugin_ids: &[String]) -> Result<Vec<String>, PlatformError> {
    let manifests = manifests_for_plugins(plugin_ids);
    let composition = handl_platform::resolve(&manifests, &[], None)
        .map_err(|e| PlatformError::Plugin(e.to_string()))?;
    let selected: HashSet<&str> = composition
        .selected_plugins
        .iter()
        .map(|s| s.as_str())
        .collect();
    Ok(GRAMMAR_RULE_PLUGIN_IDS
        .iter()
        .copied()
        .filter(|id| selected.contains(*id))
        .map(|s| s.to_string())
        .collect())
}

/// A curated kit that loads a manifest of plugins and composes a
/// lexer/parser pipeline from them.
#[derive(Debug, Default)]
pub struct Kit {
    selected_grammar_rules: Vec<String>,
}

impl Kit {
    /// Builds the canonical kit from the default manifest.
    pub fn new() -> Self {
        Self::from_manifest(&canonical_manifest()).unwrap_or_default()
    }

    /// Builds a `Kit` from a `KitManifest` by registering the listed plugins.
    pub fn from_manifest(manifest: &KitManifest) -> Result<Self, PlatformError> {
        let selected_grammar_rules = resolve_grammar_rules(&manifest.plugins)?;
        Ok(Self {
            selected_grammar_rules,
        })
    }

    /// Lexes `input` into the kit's token stream.
    pub fn lex(&self, input: &str) -> Result<Vec<Token<CanonicalKind>>, DefaultLexError> {
        CanonicalLexer.lex(input)
    }

    /// Parses `input` into a generic [`KitNode`].
    pub fn parse(&self, input: &str) -> Result<KitNode, PlatformError> {
        let tokens = self
            .lex(input)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;

        let parser = parser_for_plugins(&self.selected_grammar_rules)?;
        let ast = parser.parse(&tokens)?;
        ast_to_kitnode_with_kit(&ast, self)
    }

    /// Parses a module (a sequence of top-level statements) into a
    /// `Vec<KitNode>`.
    pub fn parse_module(&self, input: &str) -> Result<Vec<KitNode>, PlatformError> {
        let tokens = self
            .lex(input)
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;

        let parser = parser_for_plugins(&self.selected_grammar_rules)?;
        let asts = parser.parse_module(&tokens)?;
        asts.iter().map(|ast| ast_to_kitnode_with_kit(ast, self)).collect()
    }

    /// Returns the lexical grammar metadata for this kit, derived from
    /// the factory and lexer configuration. Used to generate editor grammars.
    pub fn grammar_info(&self) -> GrammarInfo {
        let config = CanonicalLexer::config();
        let factory = config.factory();

        // Partition keywords into categories based on their token kind.
        let mut booleans = Vec::new();
        let mut nulls = Vec::new();
        let mut keywords = Vec::new();
        for (text, kind) in CanonicalFactory::all_keywords() {
            match kind {
                CanonicalKind::Bool(_) => booleans.push(text.to_string()),
                CanonicalKind::Null => nulls.push(text.to_string()),
                _ => keywords.push(text.to_string()),
            }
        }

        // Derive delimiters from the lexer config.
        let delimiters: Vec<String> = config
            .delimiters()
            .iter()
            .map(|(ch, _)| ch.to_string())
            .collect();

        // Derive string delimiters from the factory.
        let string_delimiters: Vec<char> = ['"', '\'']
            .into_iter()
            .filter(|ch| factory.is_string_start(*ch))
            .collect();

        // Derive comment styles from the factory.
        let line_comment_start = factory
            .line_comment_prefix()
            .map(|s| s.to_string());
        let block_comment = factory
            .block_comment_delims()
            .map(|(s, e)| (s.to_string(), e.to_string()));
        let doc_line_comment_start = factory
            .doc_line_comment_prefix()
            .map(|s| s.to_string());
        let doc_block_comment = factory
            .doc_block_comment_delims()
            .map(|(s, e)| (s.to_string(), e.to_string()));

        GrammarInfo {
            scope_name: self.scope_name().to_string(),
            file_extensions: self.file_extensions(),
            keywords,
            booleans,
            nulls,
            operators: CanonicalFactory::all_operators(),
            delimiters,
            line_comment_start,
            block_comment,
            doc_line_comment_start,
            doc_block_comment,
            string_delimiters,
            number_regex: factory.number_regex().to_string(),
            identifier_regex: factory.identifier_regex().to_string(),
        }
    }
}

impl KitGrammarMetadata for Kit {
    fn scope_name(&self) -> &str {
        "source.handl"
    }

    fn file_extensions(&self) -> Vec<String> {
        vec!["hnl".to_string()]
    }
}

impl GrammarInfoProvider for Kit {
    fn grammar_info(&self) -> GrammarInfo {
        Kit::grammar_info(self)
    }
}

/// `Library` plugin for the canonical kit.
pub struct CanonicalKit {
    registered: AtomicBool,
}

impl CanonicalKit {
    /// Create a new `CanonicalKit`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for CanonicalKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CanonicalKit {
    fn id(&self) -> &str {
        "kit-canonical"
    }

    fn name(&self) -> &str {
        "Canonical kit"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);

        // Canonical language operation RuleAcceptors
        ctx.register_plugin(BasicLexer::new())?;
        ctx.register_plugin(BasicParser::<CanonicalKind>::new())?;
        let grammar_rules = resolve_grammar_rules(&canonical_manifest().plugins)?;
        for id in &grammar_rules {
            if let Some(rule) = rule_for_plugin_id(id) {
                ctx.contribute::<BasicParser<CanonicalKind>>(rule)?;
            }
        }
        ctx.register_plugin(BasicFormatter::new())?;
        ctx.register_plugin(BasicLinter::new())?;
        ctx.register_plugin(BasicTypechecker::new())?;
        ctx.register_plugin(BasicEvaluator::new())?;
        ctx.register_plugin(BasicBuilder::new())?;
        ctx.register_plugin(BasicCompiler::new())?;

        // Format and lint behavior contributors
        ctx.register_plugin(TriviaFormatter::new())?;
        ctx.register_plugin(StatementFormatter::new())?;
        ctx.register_plugin(ExpressionDiagnostics::new())?;
        ctx.register_plugin(HelloEvalPlugin::new())?;
        ctx.register_plugin(JsonEvalPlugin::new())?;
        ctx.register_plugin(OptionsPlugin::new())?;

        // Target and library plugins
        ctx.register_plugin(NativeTarget::new())?;

        // Runtime built-in host and families for the Rust VM target
        ctx.register_plugin(RuntimeHost::new())?;
        ctx.register_plugin(ArithmeticPlugin::new())?;
        ctx.register_plugin(BooleanPlugin::new())?;
        ctx.register_plugin(ArrayPlugin::new())?;
        ctx.register_plugin(ObjectPlugin::new())?;
        ctx.register_plugin(ApplyPlugin::new())?;

        // The canonical manifest lists token, AST-shape, and grammar-rule plugins
        // (plugin-token-*, plugin-ast-*, plugin-grammar-*). These crates provide
        // shared types and rules rather than `Plugin` implementations, so they are
        // wired directly by the existing `Kit` lexer/parser pipeline and are not
        // registered here.

        // Out-of-process plugin loaders. `LoaderHostPlugin` must be registered
        // before the loaders so their `contribute` methods can find it.
        ctx.register_plugin(LoaderHostPlugin::default())?;
        ctx.register_plugin(NativeLoader::new())?;
        ctx.register_plugin(WasmLoader::new())?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_number() {
        let kit = Kit::new();
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.len(), 2); // number + eof
        assert_eq!(tokens[0].kind, CanonicalKind::Number(42.0));
    }

    #[test]
    fn lex_string() {
        let kit = Kit::new();
        let tokens = kit.lex("\"hi\"").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::String("hi".into()));
    }

    #[test]
    fn parse_number() {
        let kit = Kit::new();
        assert_eq!(
            kit.parse("1").unwrap(),
            KitNode::Literal(KitValue::Number(1.0))
        );
    }

    #[test]
    fn parse_call() {
        let kit = Kit::new();
        let node = kit.parse("add(1, 2)").unwrap();
        let expected = KitNode::Call(Call::new(
            "add",
            vec![
                KitNode::Literal(KitValue::Number(1.0)),
                KitNode::Literal(KitValue::Number(2.0)),
            ],
        ));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_index() {
        let kit = Kit::new();
        let node = kit.parse("arr[0]").unwrap();
        let expected = KitNode::Index(Box::new(Index::new(
            KitNode::Var(Var::new("arr".into())),
            KitNode::Literal(KitValue::Number(0.0)),
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_pipe() {
        let kit = Kit::new();
        let node = kit.parse("1 |> add(2, 3)").unwrap();
        let right = KitNode::Call(Call::new(
            "add",
            vec![
                KitNode::Literal(KitValue::Number(2.0)),
                KitNode::Literal(KitValue::Number(3.0)),
            ],
        ));
        let expected = KitNode::Pipe(Box::new(Pipe::new(
            KitNode::Literal(KitValue::Number(1.0)),
            right,
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_let() {
        let kit = Kit::new();
        let node = kit.parse("a = 1").unwrap();
        let expected = KitNode::Assign(Box::new(Assign::new(
            KitNode::Var(Var::new("a".into())),
            KitNode::Literal(KitValue::Number(1.0)),
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_string_literal() {
        let kit = Kit::new();
        let node = kit.parse("\"hi\"").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String("hi".into())));
    }

    #[test]
    fn parse_bool_literal() {
        let kit = Kit::new();
        assert_eq!(
            kit.parse("true").unwrap(),
            KitNode::Literal(KitValue::Bool(true))
        );
        assert_eq!(
            kit.parse("false").unwrap(),
            KitNode::Literal(KitValue::Bool(false))
        );
    }

    #[test]
    fn parse_null_literal() {
        let kit = Kit::new();
        assert_eq!(kit.parse("null").unwrap(), KitNode::Literal(KitValue::Null));
    }

    #[test]
    fn parse_module_empty() {
        let kit = Kit::new();
        let stmts = kit.parse_module("").unwrap();
        assert!(stmts.is_empty());
    }

    #[test]
    fn parse_module_single_let() {
        let kit = Kit::new();
        let stmts = kit.parse_module("a = 1;").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Assign(Box::new(Assign::new(
            KitNode::Var(Var::new("a".into())),
            KitNode::Literal(KitValue::Number(1.0)),
        )));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_multiple_let() {
        let kit = Kit::new();
        let stmts = kit.parse_module("a = 1; b = 2;").unwrap();
        assert_eq!(stmts.len(), 2);
        assert_eq!(
            stmts[0],
            KitNode::Assign(Box::new(Assign::new(
                KitNode::Var(Var::new("a".into())),
                KitNode::Literal(KitValue::Number(1.0)),
            )))
        );
        assert_eq!(
            stmts[1],
            KitNode::Assign(Box::new(Assign::new(
                KitNode::Var(Var::new("b".into())),
                KitNode::Literal(KitValue::Number(2.0)),
            )))
        );
    }

    #[test]
    fn parse_module_const() {
        let kit = Kit::new();
        let stmts = kit.parse_module("x: number = 42;").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Let(Box::new(Let::new(
            "x".into(),
            KitNode::Literal(KitValue::Number(42.0)),
        )));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_expression_statement() {
        let kit = Kit::new();
        let stmts = kit.parse_module("42").unwrap();
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn parse_module_mixed() {
        let kit = Kit::new();
        let stmts = kit.parse_module("a = 1; 42").unwrap();
        assert_eq!(stmts.len(), 2);
        assert_eq!(
            stmts[0],
            KitNode::Assign(Box::new(Assign::new(
                KitNode::Var(Var::new("a".into())),
                KitNode::Literal(KitValue::Number(1.0)),
            )))
        );
        assert_eq!(stmts[1], KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn parse_module_import() {
        let kit = Kit::new();
        let stmts = kit.parse_module("import \"./lib\"").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Import(Box::new(Import::new(KitNode::Literal(KitValue::String(
            "./lib".into(),
        )))));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_export() {
        let kit = Kit::new();
        let stmts = kit.parse_module("export 42").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Export(Box::new(Export::new(KitNode::Literal(KitValue::Number(
            42.0,
        )))));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_module_decl() {
        let kit = Kit::new();
        let stmts = kit.parse_module("module mymod").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Module(Box::new(Module::new("mymod".into(), None)));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_module_module_with_value() {
        let kit = Kit::new();
        let stmts = kit.parse_module("module mymod = 42").unwrap();
        assert_eq!(stmts.len(), 1);
        let expected = KitNode::Module(Box::new(Module::new(
            "mymod".into(),
            Some(KitNode::Literal(KitValue::Number(42.0))),
        )));
        assert_eq!(stmts[0], expected);
    }

    #[test]
    fn parse_apply() {
        let kit = Kit::new();
        let node = kit.parse("(1)").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::Number(1.0)));
    }

    #[test]
    fn parse_assign() {
        let kit = Kit::new();
        let node = kit.parse("a = 1").unwrap();
        let expected = KitNode::Assign(Box::new(Assign::new(
            KitNode::Var(Var::new("a".into())),
            KitNode::Literal(KitValue::Number(1.0)),
        )));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_array() {
        let kit = Kit::new();
        let node = kit.parse("[1, 2]").unwrap();
        let expected = KitNode::Array(Array::new(vec![
            KitNode::Literal(KitValue::Number(1.0)),
            KitNode::Literal(KitValue::Number(2.0)),
        ]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_object() {
        let kit = Kit::new();
        let node = kit.parse("{a: 1}").unwrap();
        let expected = KitNode::Object(Object::new(vec![(
            "a".into(),
            KitNode::Literal(KitValue::Number(1.0)),
        )]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_match() {
        let kit = Kit::new();
        let node = kit.parse("match x").unwrap();
        let expected = KitNode::Match(Box::new(Match::new(KitNode::Var(Var::new("x".into())))));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_handle() {
        let kit = Kit::new();
        let node = kit.parse("handle x").unwrap();
        let expected = KitNode::Handle(Box::new(Handle::new(KitNode::Var(Var::new("x".into())))));
        assert_eq!(node, expected);
    }

    #[test]
    fn from_manifest_subset_parses_values_but_not_calls() {
        let manifest = KitManifest {
            id: "subset".to_string(),
            version: "0.1.0".to_string(),
            plugins: vec![
                "plugin-grammar-primary-value".to_string(),
                "plugin-grammar-type-annot".to_string(),
                "plugin-grammar-stmt-import".to_string(),
            ],
        };
        let kit = Kit::from_manifest(&manifest).unwrap();
        assert_eq!(
            kit.parse("42").unwrap(),
            KitNode::Literal(KitValue::Number(42.0))
        );
        assert!(kit.parse("[1, 2]").is_err());
    }

    #[test]
    fn canonical_manifest_passes_ast_kind_validation() {
        let manifest = canonical_manifest();
        let manifests = manifests_for_plugins(&manifest.plugins);
        let selected: HashSet<String> = manifests.iter().map(|m| m.id.clone()).collect();
        assert!(handl_platform::resolve::validate_ast_kinds(&manifests, &selected).is_ok());
    }

    #[test]
    fn subset_missing_producer_fails_ast_kind_validation() {
        let grammar_call = manifests_for_plugins(&["plugin-grammar-call".to_string()])[0].clone();
        let grammar_value =
            manifests_for_plugins(&["plugin-grammar-primary-value".to_string()])[0].clone();
        let mut consumer = grammar_call.clone();
        consumer.id = "consumer-of-call".to_string();
        consumer.ast_kinds = vec![AstKindDeclaration {
            kind: "call".into(),
            role: AstKindRole::Consumes,
        }];
        let all = vec![grammar_call, grammar_value, consumer];
        let selected: HashSet<String> = ["plugin-grammar-primary-value", "consumer-of-call"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let err = handl_platform::resolve::validate_ast_kinds(&all, &selected).unwrap_err();
        assert!(matches!(
            err,
            handl_platform::resolve::ResolutionError::MissingProducer { ref kind, ref consumed_by, .. }
            if kind == "call" && consumed_by == "consumer-of-call"
        ));
    }

    // --- Template string integration tests ---

    #[test]
    fn lex_template_simple() {
        let kit = Kit::new();
        let tokens = kit.lex("`hello`").unwrap();
        assert_eq!(tokens.len(), 2); // template + eof
        assert_eq!(
            tokens[0].kind,
            CanonicalKind::Template(vec![TemplateSegment::Text("hello".into())])
        );
    }

    #[test]
    fn lex_template_with_interpolation() {
        let kit = Kit::new();
        let tokens = kit.lex("`hi ${name}!`").unwrap();
        assert_eq!(
            tokens[0].kind,
            CanonicalKind::Template(vec![
                TemplateSegment::Text("hi ".into()),
                TemplateSegment::Expr("name".into()),
                TemplateSegment::Text("!".into()),
            ])
        );
    }

    #[test]
    fn lex_template_empty() {
        let kit = Kit::new();
        let tokens = kit.lex("``").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Template(vec![]));
    }

    #[test]
    fn lex_template_multiple_interpolations() {
        let kit = Kit::new();
        let tokens = kit.lex("`${a} and ${b}`").unwrap();
        assert_eq!(
            tokens[0].kind,
            CanonicalKind::Template(vec![
                TemplateSegment::Expr("a".into()),
                TemplateSegment::Text(" and ".into()),
                TemplateSegment::Expr("b".into()),
            ])
        );
    }

    #[test]
    fn lex_template_unterminated_errors() {
        let kit = Kit::new();
        assert!(kit.lex("`oops").is_err());
    }

    #[test]
    fn lex_template_unterminated_interpolation_errors() {
        let kit = Kit::new();
        assert!(kit.lex("`hi ${name").is_err());
    }

    #[test]
    fn parse_template_text_only() {
        let kit = Kit::new();
        let node = kit.parse("`hello`").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String("hello".into())));
    }

    #[test]
    fn parse_template_empty() {
        let kit = Kit::new();
        let node = kit.parse("``").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String(String::new())));
    }

    #[test]
    fn parse_template_expr_only() {
        let kit = Kit::new();
        let node = kit.parse("`${42}`").unwrap();
        // Only one part — returns it directly (no concat needed)
        assert_eq!(node, KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn parse_template_text_and_expr() {
        let kit = Kit::new();
        let node = kit.parse("`hello ${42}!`").unwrap();
        // str.concat("hello ", str.concat(42, "!"))
        let expected = KitNode::Call(Call::new("str.concat", vec![
            KitNode::Literal(KitValue::String("hello ".into())),
            KitNode::Call(Call::new("str.concat", vec![
                KitNode::Literal(KitValue::Number(42.0)),
                KitNode::Literal(KitValue::String("!".into())),
            ])),
        ]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_template_multiple_interpolations() {
        let kit = Kit::new();
        let node = kit.parse("`${1}${2}`").unwrap();
        // str.concat(1, 2)
        let expected = KitNode::Call(Call::new("str.concat", vec![
            KitNode::Literal(KitValue::Number(1.0)),
            KitNode::Literal(KitValue::Number(2.0)),
        ]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_template_complex() {
        let kit = Kit::new();
        let node = kit.parse("`a${1}b${2}c`").unwrap();
        // str.concat("a", str.concat(1, str.concat("b", str.concat(2, "c"))))
        let expected = KitNode::Call(Call::new("str.concat", vec![
            KitNode::Literal(KitValue::String("a".into())),
            KitNode::Call(Call::new("str.concat", vec![
                KitNode::Literal(KitValue::Number(1.0)),
                KitNode::Call(Call::new("str.concat", vec![
                    KitNode::Literal(KitValue::String("b".into())),
                    KitNode::Call(Call::new("str.concat", vec![
                        KitNode::Literal(KitValue::Number(2.0)),
                        KitNode::Literal(KitValue::String("c".into())),
                    ])),
                ])),
            ])),
        ]));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_template_with_string_expr() {
        let kit = Kit::new();
        let node = kit.parse("`${\"inner\"}`").unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String("inner".into())));
    }

    #[test]
    fn parse_template_with_var_expr() {
        let kit = Kit::new();
        let node = kit.parse("`${name}`").unwrap();
        assert_eq!(node, KitNode::Var(Var::new("name".into())));
    }

    #[test]
    fn parse_template_multiline() {
        let kit = Kit::new();
        let node = kit.parse("`line1\nline2`").unwrap();
        assert_eq!(
            node,
            KitNode::Literal(KitValue::String("line1\nline2".into()))
        );
    }

    #[test]
    fn canonical_kind_template_display() {
        let kind = CanonicalKind::Template(vec![
            TemplateSegment::Text("hello ".into()),
            TemplateSegment::Expr("name".into()),
        ]);
        let s = format!("{kind}");
        assert!(s.starts_with("template("));
        assert!(s.contains("\"hello \""));
        assert!(s.contains("${name}"));
        assert!(s.ends_with(")"));
    }

    #[test]
    fn canonical_kind_template_display_text_only() {
        let kind = CanonicalKind::Template(vec![TemplateSegment::Text("hi".into())]);
        let s = format!("{kind}");
        assert_eq!(s, "template(\"hi\")");
    }

    #[test]
    fn canonical_kind_template_display_empty() {
        let kind = CanonicalKind::Template(vec![]);
        let s = format!("{kind}");
        assert_eq!(s, "template()");
    }

    #[test]
    fn template_token_kind_constructs() {
        let kind = CanonicalKind::template_kind(vec![TemplateSegment::Text("test".into())]);
        assert!(matches!(kind, CanonicalKind::Template(_)));
    }

    #[test]
    fn template_token_returns_segments() {
        let kind = CanonicalKind::Template(vec![
            TemplateSegment::Text("a".into()),
            TemplateSegment::Expr("b".into()),
        ]);
        let segs = kind.as_template_segments().unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], TemplateSegment::Text("a".into()));
        assert_eq!(segs[1], TemplateSegment::Expr("b".into()));
    }

    #[test]
    fn template_token_returns_none_for_non_template() {
        assert!(CanonicalKind::Number(1.0).as_template_segments().is_none());
        assert!(CanonicalKind::String("x".into()).as_template_segments().is_none());
        assert!(CanonicalKind::Eof.as_template_segments().is_none());
    }

    #[test]
    fn ast_to_kitnode_with_kit_missing_segments_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({}),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_missing_type_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "value": "hi" })]
            }),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_unknown_segment_type_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "bogus" })]
            }),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_missing_value_in_text_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "text" })]
            }),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_missing_raw_in_expr_errors() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "expr" })]
            }),
        };
        let kit = Kit::new();
        let result = ast_to_kitnode_with_kit(&ast, &kit);
        assert!(result.is_err());
    }

    #[test]
    fn ast_to_kitnode_with_kit_empty_segments() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({ "segments": [] }),
        };
        let kit = Kit::new();
        let node = ast_to_kitnode_with_kit(&ast, &kit).unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String(String::new())));
    }

    #[test]
    fn ast_to_kitnode_with_kit_single_text() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "text", "value": "hello" })]
            }),
        };
        let kit = Kit::new();
        let node = ast_to_kitnode_with_kit(&ast, &kit).unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::String("hello".into())));
    }

    #[test]
    fn ast_to_kitnode_with_kit_single_expr() {
        let ast = AstNode {
            kind: "template".into(),
            span: None,
            payload: serde_json::json!({
                "segments": [serde_json::json!({ "type": "expr", "raw": "42" })]
            }),
        };
        let kit = Kit::new();
        let node = ast_to_kitnode_with_kit(&ast, &kit).unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn ast_to_kitnode_with_kit_delegates_non_template() {
        let ast = AstNode {
            kind: "value".into(),
            span: None,
            payload: serde_json::json!({ "value": 42 }),
        };
        let kit = Kit::new();
        let node = ast_to_kitnode_with_kit(&ast, &kit).unwrap();
        assert_eq!(node, KitNode::Literal(KitValue::Number(42.0)));
    }

    #[test]
    fn canonical_kit_registers_loaders() {
        use handl_platform::{DefaultHost, Host};

        let mut host = DefaultHost::new();
        host.register_plugin(CanonicalKit::new());
        host.load().unwrap();

        assert!(host.plugins().has_id("loader.host"));
        assert!(host.plugins().has_id("loader.native"));
        assert!(host.plugins().has_id("loader.wasm"));
        assert!(host.plugins().resolve(&"load".into()).is_some());
    }
}
