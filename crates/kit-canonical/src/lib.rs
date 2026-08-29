//! Canonical kit as a generic plugin host.
//!
//! `Kit` loads a [`KitManifest`], registers the core canonical plugins, and
//! composes a [`Lexer`], a parser, and a node factory from them. It no longer
//! depends on the concrete `handl-ast-canonical`, `handl-lexer-canonical`, or
//! `handl-parser-canonical` crates.

use std::fmt;
use std::iter::Peekable;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use handl_plugin_ast::{Expr as ExprTrait, Node, Stmt as StmtTrait};
use handl_plugin_lexer::primitives;
use handl_plugin_lexer::{
    CommentFactory, CommentTokenKind, DefaultLexError, EofFactory, IdentFactory, KeywordFactory,
    KindFactory, Lexer, LexerConfig, NumberFactory, OperatorFactory, Span, StringFactory, Token,
    TokenKind,
};
use handl_plugin_lexer_block_comment::BlockCommentRule;
use handl_plugin_lexer_delimiter::DelimiterRule;
use handl_plugin_lexer_ident::IdentRule;
use handl_plugin_lexer_keyword::KeywordRule;
use handl_plugin_lexer_line_comment::LineCommentRule;
use handl_plugin_lexer_number::NumberRule as LexNumberRule;
use handl_plugin_lexer_operator::OperatorRule;
use handl_plugin_lexer_string::StringRule as LexStringRule;
use handl_parser::{CompositeGrammar, Grammar, GrammarRule, ParseError, ParserContext};
use handl_platform::{
    BasicEvaluator, BasicTypechecker, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, OptionsPlugin, PlatformError, Plugin, PluginContext,
};
use handl_plugin_ext_lexer::BasicLexer;
use handl_plugin_ext_parser::BasicParser;
use handl_plugin_ext_build::BasicBuilder;
use handl_plugin_ext_compile::BasicCompiler;
use handl_plugin_ext_fmt::BasicFormatter;
use handl_plugin_ext_lint::BasicLinter;
use handl_plugin_diag_expression::ExpressionDiagnostics;
use handl_plugin_fmt_stmt::StatementFormatter;
use handl_plugin_fmt_trivia::TriviaFormatter;

use handl_target_native::NativeTarget;
use handl_platform::RuntimeHost;
use handl_plugin_eval_hello::HelloEvalPlugin;
use handl_plugin_eval_json::JsonEvalPlugin;
use handl_plugin_runtime_arithmetic::ArithmeticPlugin;
use handl_plugin_runtime_array::ArrayPlugin;
use handl_plugin_runtime_apply::ApplyPlugin;
use handl_plugin_runtime_boolean::BooleanPlugin;
use handl_plugin_runtime_object::ObjectPlugin;
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
use handl_plugin_grammar_apply::ApplyToken;
use handl_plugin_grammar_array::{ArrayRule, ArrayToken};
use handl_plugin_grammar_assign::{AssignRule, AssignToken};
use handl_plugin_grammar_call::{CallRule, CallToken};
use handl_plugin_grammar_handle::{HandleRule, HandleToken};
use handl_plugin_grammar_index::{IndexRule, IndexToken};
use handl_plugin_grammar_let::{LetRule, LetToken};
use handl_plugin_grammar_match::{MatchRule, MatchToken};
use handl_plugin_grammar_object::{ObjectRule, ObjectToken};
use handl_plugin_grammar_pipe::{PipeRule, PipeToken};
use handl_plugin_token_bool::BoolKind;
use handl_plugin_token_comment::CommentKind;
use handl_plugin_token_doc_comment::{
    is_doc_block_comment_start, is_doc_line_comment_start, lex_doc_block_comment,
    lex_doc_line_comment, DocCommentKind, DocCommentRule,
};
use handl_plugin_token_delimiter::DelimiterKind;
use handl_plugin_token_identifier::IdentKind;
use handl_plugin_token_keyword_as::AsKind;
use handl_plugin_token_keyword_handle::HandleKind;
use handl_plugin_token_null::NullKind;
use handl_plugin_token_number::NumberKind;
use handl_plugin_token_operator::{OperatorKind, CANONICAL_OPERATOR_CHARS};
use handl_plugin_token_string::StringKind;

pub mod diagnostics;
pub use diagnostics::{Diagnostic, DiagnosticLevel, Diagnostics};

pub mod statement;
pub use statement::{expr_to_pattern, ModuleOutput, Stmt, StmtKind};

pub mod kit_manifest;
pub use kit_manifest::{canonical_manifest, KitManifest};

/// A generic metadata value for kit nodes.
pub type KitMetadata = ();

static UNIT: KitMetadata = ();

/// Canonical keyword forms used by the kit token kind.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalKeyword {
    Let,
    Const,
    Match,
    Handle,
    As,
    TypeOf,
    Not,
}

/// Canonical delimiter forms.
#[derive(Debug, Clone, PartialEq)]
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

/// Kit-level unified token kind for the canonical dialect.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalKind {
    /// A numeric literal.
    Number(f64),
    /// A string literal.
    String(String),
    /// An identifier or path.
    Identifier(String),
    /// A boolean literal.
    Bool(bool),
    /// The `null` literal.
    Null,
    /// A comment.
    Comment(String),
    /// A doc comment.
    DocComment(String),
    /// A delimiter: `(`, `)`, `[`, `]`, `{`, `}`, `,`, `:`, `.`, `;`.
    Delimiter(CanonicalDelim),
    /// An operator: `|>`, `->`, `=`, `+`, `-`, `*`, `/`, `<`, `>`.
    Operator(CanonicalOp),
    /// A keyword.
    Keyword(CanonicalKeyword),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for CanonicalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanonicalKind::Number(n) => write!(f, "number({n})"),
            CanonicalKind::String(s) => write!(f, "string({s:?})"),
            CanonicalKind::Identifier(s) => write!(f, "identifier({s})"),
            CanonicalKind::Bool(b) => write!(f, "bool({b})"),
            CanonicalKind::Null => write!(f, "null"),
            CanonicalKind::Comment(s) => write!(f, "comment({s:?})"),
            CanonicalKind::DocComment(s) => write!(f, "doc-comment({s:?})"),
            CanonicalKind::Delimiter(d) => match d {
                CanonicalDelim::ParenOpen => write!(f, "("),
                CanonicalDelim::ParenClose => write!(f, ")"),
                CanonicalDelim::BracketOpen => write!(f, "["),
                CanonicalDelim::BracketClose => write!(f, "]"),
                CanonicalDelim::BraceOpen => write!(f, "{{"),
                CanonicalDelim::BraceClose => write!(f, "}}"),
                CanonicalDelim::Comma => write!(f, ","),
                CanonicalDelim::Colon => write!(f, ":"),
                CanonicalDelim::Dot => write!(f, "."),
                CanonicalDelim::Semicolon => write!(f, ";"),
            },
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
            CanonicalKind::Keyword(k) => match k {
                CanonicalKeyword::Let => write!(f, "let"),
                CanonicalKeyword::Const => write!(f, "const"),
                CanonicalKeyword::Match => write!(f, "match"),
                CanonicalKeyword::Handle => write!(f, "handle"),
                CanonicalKeyword::As => write!(f, "as"),
                CanonicalKeyword::TypeOf => write!(f, "typeof"),
                CanonicalKeyword::Not => write!(f, "not"),
            },
            CanonicalKind::Eof => write!(f, "eof"),
        }
    }
}

/// The factory that turns raw values into [`CanonicalKind`] tokens.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonicalFactory;

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
        match text {
            "true" => Some(CanonicalKind::Bool(true)),
            "false" => Some(CanonicalKind::Bool(false)),
            "null" => Some(CanonicalKind::Null),
            "let" => Some(CanonicalKind::Keyword(CanonicalKeyword::Let)),
            "const" => Some(CanonicalKind::Keyword(CanonicalKeyword::Const)),
            "match" => Some(CanonicalKind::Keyword(CanonicalKeyword::Match)),
            "handle" => Some(CanonicalKind::Keyword(CanonicalKeyword::Handle)),
            "as" => Some(CanonicalKind::Keyword(CanonicalKeyword::As)),
            "typeof" => Some(CanonicalKind::Keyword(CanonicalKeyword::TypeOf)),
            "not" => Some(CanonicalKind::Keyword(CanonicalKeyword::Not)),
            _ => None,
        }
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

    fn lex_doc_line_comment(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> (String, Span) {
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

impl TokenKind for CanonicalKind {
    type Factory = CanonicalFactory;

    fn is_eof(&self) -> bool {
        matches!(self, CanonicalKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }
}

impl CommentTokenKind for CanonicalKind {
    fn is_comment(&self) -> bool {
        matches!(self, CanonicalKind::Comment(_) | CanonicalKind::DocComment(_))
    }
}

/// Conversions from the individual `plugin-token-*` kinds into the kit-level
/// unified [`CanonicalKind`]. These are the "wires" that let the kit compose
/// the token plugins into a single token stream.
impl From<NumberKind> for CanonicalKind {
    fn from(k: NumberKind) -> Self {
        match k {
            NumberKind::Number(v) => CanonicalKind::Number(v),
            NumberKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<StringKind> for CanonicalKind {
    fn from(k: StringKind) -> Self {
        match k {
            StringKind::String(v) => CanonicalKind::String(v),
            StringKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<IdentKind> for CanonicalKind {
    fn from(k: IdentKind) -> Self {
        match k {
            IdentKind::Ident(v) => CanonicalKind::Identifier(v),
            IdentKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<BoolKind> for CanonicalKind {
    fn from(k: BoolKind) -> Self {
        match k {
            BoolKind::Bool(v) => CanonicalKind::Bool(v),
            BoolKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<NullKind> for CanonicalKind {
    fn from(k: NullKind) -> Self {
        match k {
            NullKind::Null => CanonicalKind::Null,
            NullKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<CommentKind> for CanonicalKind {
    fn from(k: CommentKind) -> Self {
        match k {
            CommentKind::Comment(v) => CanonicalKind::Comment(v),
            CommentKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<DocCommentKind> for CanonicalKind {
    fn from(k: DocCommentKind) -> Self {
        match k {
            DocCommentKind::DocComment(v) => CanonicalKind::DocComment(v),
            DocCommentKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<DelimiterKind> for CanonicalKind {
    fn from(k: DelimiterKind) -> Self {
        match k {
            DelimiterKind::ParenOpen => CanonicalKind::Delimiter(CanonicalDelim::ParenOpen),
            DelimiterKind::ParenClose => CanonicalKind::Delimiter(CanonicalDelim::ParenClose),
            DelimiterKind::BracketOpen => CanonicalKind::Delimiter(CanonicalDelim::BracketOpen),
            DelimiterKind::BracketClose => CanonicalKind::Delimiter(CanonicalDelim::BracketClose),
            DelimiterKind::BraceOpen => CanonicalKind::Delimiter(CanonicalDelim::BraceOpen),
            DelimiterKind::BraceClose => CanonicalKind::Delimiter(CanonicalDelim::BraceClose),
            DelimiterKind::Comma => CanonicalKind::Delimiter(CanonicalDelim::Comma),
            DelimiterKind::Colon => CanonicalKind::Delimiter(CanonicalDelim::Colon),
            DelimiterKind::Dot => CanonicalKind::Delimiter(CanonicalDelim::Dot),
            DelimiterKind::Semicolon => CanonicalKind::Delimiter(CanonicalDelim::Semicolon),
            DelimiterKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<OperatorKind> for CanonicalKind {
    fn from(k: OperatorKind) -> Self {
        match k {
            OperatorKind::Pipe => CanonicalKind::Operator(CanonicalOp::Pipe),
            OperatorKind::Arrow => CanonicalKind::Operator(CanonicalOp::Arrow),
            OperatorKind::Equal => CanonicalKind::Operator(CanonicalOp::Equal),
            OperatorKind::Plus => CanonicalKind::Operator(CanonicalOp::Plus),
            OperatorKind::Minus => CanonicalKind::Operator(CanonicalOp::Minus),
            OperatorKind::Star => CanonicalKind::Operator(CanonicalOp::Star),
            OperatorKind::Slash => CanonicalKind::Operator(CanonicalOp::Slash),
            OperatorKind::LessThan => CanonicalKind::Operator(CanonicalOp::LessThan),
            OperatorKind::GreaterThan => CanonicalKind::Operator(CanonicalOp::GreaterThan),
            OperatorKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<AsKind> for CanonicalKind {
    fn from(k: AsKind) -> Self {
        match k {
            AsKind::As => CanonicalKind::Keyword(CanonicalKeyword::As),
            AsKind::Eof => CanonicalKind::Eof,
        }
    }
}

impl From<HandleKind> for CanonicalKind {
    fn from(k: HandleKind) -> Self {
        match k {
            HandleKind::Handle => CanonicalKind::Keyword(CanonicalKeyword::Handle),
            HandleKind::Eof => CanonicalKind::Eof,
        }
    }
}

// Token capability implementations for the grammar-rule plugins.
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

impl LetToken for CanonicalKind {
    fn is_let(&self) -> bool {
        matches!(self, CanonicalKind::Keyword(CanonicalKeyword::Let))
    }

    fn as_let_name(&self) -> Option<&str> {
        match self {
            CanonicalKind::Identifier(name) => Some(name.as_str()),
            _ => None,
        }
    }

    fn is_let_equal(&self) -> bool {
        matches!(self, CanonicalKind::Operator(CanonicalOp::Equal))
    }

    fn is_let_semicolon(&self) -> bool {
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
    /// A string value.
    String(String),
    /// A boolean value.
    Bool(bool),
    /// The `null` value.
    Null,
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

/// Built-in grammar rule for parsing numeric literals.
#[derive(Debug, Default, Clone, Copy)]
pub struct NumberRule;

impl GrammarRule<CanonicalKind, KitNode> for NumberRule {
    fn matches_prefix(&self, kind: &CanonicalKind) -> bool {
        matches!(kind, CanonicalKind::Number(_))
    }

    fn prefix(
        &self,
        token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
    ) -> Result<KitNode, ParseError> {
        match token.kind {
            CanonicalKind::Number(v) => Ok(KitNode::Literal(KitValue::Number(v))),
            _ => Err(ParseError::UnexpectedToken {
                span: token.span,
                expected: "number literal".into(),
            }),
        }
    }

    fn infix_binding(&self, _kind: &CanonicalKind) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: KitNode,
        _token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
        _rbp: u32,
    ) -> Result<KitNode, ParseError> {
        panic!("NumberRule is prefix-only")
    }
}

/// Built-in grammar rule for parsing string literals.
#[derive(Debug, Default, Clone, Copy)]
pub struct StringRule;

impl GrammarRule<CanonicalKind, KitNode> for StringRule {
    fn matches_prefix(&self, kind: &CanonicalKind) -> bool {
        matches!(kind, CanonicalKind::String(_))
    }

    fn prefix(
        &self,
        token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
    ) -> Result<KitNode, ParseError> {
        match token.kind {
            CanonicalKind::String(s) => Ok(KitNode::Literal(KitValue::String(s))),
            _ => Err(ParseError::UnexpectedToken {
                span: token.span,
                expected: "string literal".into(),
            }),
        }
    }

    fn infix_binding(&self, _kind: &CanonicalKind) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: KitNode,
        _token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
        _rbp: u32,
    ) -> Result<KitNode, ParseError> {
        panic!("StringRule is prefix-only")
    }
}

/// Built-in grammar rule for parsing boolean literals.
#[derive(Debug, Default, Clone, Copy)]
pub struct BoolRule;

impl GrammarRule<CanonicalKind, KitNode> for BoolRule {
    fn matches_prefix(&self, kind: &CanonicalKind) -> bool {
        matches!(kind, CanonicalKind::Bool(_))
    }

    fn prefix(
        &self,
        token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
    ) -> Result<KitNode, ParseError> {
        match token.kind {
            CanonicalKind::Bool(b) => Ok(KitNode::Literal(KitValue::Bool(b))),
            _ => Err(ParseError::UnexpectedToken {
                span: token.span,
                expected: "boolean literal".into(),
            }),
        }
    }

    fn infix_binding(&self, _kind: &CanonicalKind) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: KitNode,
        _token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
        _rbp: u32,
    ) -> Result<KitNode, ParseError> {
        panic!("BoolRule is prefix-only")
    }
}

/// Built-in grammar rule for parsing `null`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullRule;

impl GrammarRule<CanonicalKind, KitNode> for NullRule {
    fn matches_prefix(&self, kind: &CanonicalKind) -> bool {
        matches!(kind, CanonicalKind::Null)
    }

    fn prefix(
        &self,
        token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
    ) -> Result<KitNode, ParseError> {
        match token.kind {
            CanonicalKind::Null => Ok(KitNode::Literal(KitValue::Null)),
            _ => Err(ParseError::UnexpectedToken {
                span: token.span,
                expected: "null".into(),
            }),
        }
    }

    fn infix_binding(&self, _kind: &CanonicalKind) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: KitNode,
        _token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
        _rbp: u32,
    ) -> Result<KitNode, ParseError> {
        panic!("NullRule is prefix-only")
    }
}

/// Built-in grammar rule for parsing identifiers.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdentifierRule;

impl GrammarRule<CanonicalKind, KitNode> for IdentifierRule {
    fn matches_prefix(&self, kind: &CanonicalKind) -> bool {
        matches!(kind, CanonicalKind::Identifier(_))
    }

    fn prefix(
        &self,
        token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
    ) -> Result<KitNode, ParseError> {
        match token.kind {
            CanonicalKind::Identifier(name) => Ok(KitNode::Var(Var::new(name))),
            _ => Err(ParseError::UnexpectedToken {
                span: token.span,
                expected: "identifier".into(),
            }),
        }
    }

    fn infix_binding(&self, _kind: &CanonicalKind) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: KitNode,
        _token: Token<CanonicalKind>,
        _ctx: &mut dyn ParserContext<CanonicalKind, KitNode>,
        _rbp: u32,
    ) -> Result<KitNode, ParseError> {
        panic!("IdentifierRule is prefix-only")
    }
}

/// The kit's concrete lexer.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonicalLexer;

impl Lexer<CanonicalKind, DefaultLexError> for CanonicalLexer {
    fn lex(&self, input: &str) -> Result<Vec<Token<CanonicalKind>>, DefaultLexError> {
        let tokens = LexerConfig::new(CanonicalFactory)
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
            .with_rule(LexNumberRule)
            .with_rule(KeywordRule)
            .with_rule(IdentRule)
            .with_rule(OperatorRule)
            .with_rule(DelimiterRule)
            .lex(input)?;

        Ok(tokens.into_iter().filter(|t| !t.kind.is_comment()).collect())
    }
}

struct KitParserContext {
    tokens: Vec<Token<CanonicalKind>>,
    source: String,
    pos: usize,
    grammar: Arc<dyn Grammar<CanonicalKind, KitNode>>,
}

impl KitParserContext {
    fn peek_span(&self) -> Span {
        self.peek()
            .map(|t| t.span)
            .unwrap_or_else(|| Span::at(self.tokens.last().map(|t| t.span.end).unwrap_or(0)))
    }
}

impl ParserContext<CanonicalKind, KitNode> for KitParserContext {
    fn tokens(&self) -> &[Token<CanonicalKind>] {
        &self.tokens
    }

    fn source(&self) -> &str {
        &self.source
    }

    fn pos(&self) -> usize {
        self.pos
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token<CanonicalKind>> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<CanonicalKind> {
        self.peek().map(|t| t.kind.clone())
    }

    fn advance(&mut self) -> Result<&Token<CanonicalKind>, ParseError> {
        if self.tokens.get(self.pos).is_some() {
            let pos = self.pos;
            self.pos += 1;
            return Ok(&self.tokens[pos]);
        }
        Err(ParseError::UnexpectedEof {
            span: self.peek_span(),
            expected: "a token".into(),
        })
    }

    fn match_kind(&mut self, kind: CanonicalKind) -> Option<&Token<CanonicalKind>> {
        if self.tokens.get(self.pos).map_or(false, |t| t.kind == kind) {
            let pos = self.pos;
            self.pos += 1;
            return self.tokens.get(pos);
        }
        None
    }

    fn expect(&mut self, kind: CanonicalKind, label: &str) -> Result<&Token<CanonicalKind>, ParseError> {
        let span = self.peek_span();
        match self.match_kind(kind) {
            Some(token) => Ok(token),
            None => Err(ParseError::UnexpectedToken {
                span,
                expected: label.into(),
            }),
        }
    }

    fn expression(&mut self, min_bp: u32) -> Result<KitNode, ParseError> {
        let token = self.advance()?.clone();
        let grammar = self.grammar.clone();
        let rule = grammar
            .rule_for_prefix(&token.kind)
            .ok_or_else(|| ParseError::UnexpectedToken {
                span: token.span,
                expected: "a prefix expression".into(),
            })?;
        let mut left = rule.prefix(token, self)?;

        while let Some(kind) = self.peek_kind() {
            let grammar = self.grammar.clone();
            let rule = match grammar.rule_for_infix(&kind) {
                Some(rule) => rule,
                None => break,
            };
            let (left_bp, right_bp) = match rule.infix_binding(&kind) {
                Some(bp) => bp,
                None => break,
            };
            if left_bp <= min_bp {
                break;
            }
            let op = self.advance()?.clone();
            left = rule.infix(left, op, self, right_bp)?;
        }

        Ok(left)
    }
}

/// The kit's parser, built on `handl_parser::CompositeGrammar`.
pub struct KitParser {
    grammar: Arc<dyn Grammar<CanonicalKind, KitNode>>,
}

impl KitParser {
    fn new(grammar: CompositeGrammar<CanonicalKind, KitNode>) -> Self {
        let grammar: Arc<dyn Grammar<CanonicalKind, KitNode>> = Arc::new(grammar);
        Self { grammar }
    }

    fn parse(&self, tokens: Vec<Token<CanonicalKind>>) -> Result<KitNode, ParseError> {
        let eof = self.grammar.eof();
        let mut ctx = KitParserContext {
            tokens,
            source: String::new(),
            pos: 0,
            grammar: self.grammar.clone(),
        };
        let root = self.grammar.parse_root(&mut ctx)?;

        if let Some(eof) = eof {
            if ctx.match_kind(eof).is_none() && !ctx.is_at_end() {
                return Err(ParseError::UnexpectedToken {
                    span: ctx.peek_span(),
                    expected: "end of input".into(),
                });
            }
        } else if !ctx.is_at_end() {
            return Err(ParseError::UnexpectedToken {
                span: ctx.peek_span(),
                expected: "end of input".into(),
            });
        }

        Ok(root)
    }
}

/// A curated kit that loads a manifest of plugins and composes a
/// lexer/parser pipeline from them.
#[derive(Debug, Default)]
pub struct Kit;

impl Kit {
    /// Builds the canonical kit from the default manifest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a `Kit` from a `KitManifest` by registering the listed plugins.
    pub fn from_manifest(_manifest: &KitManifest) -> Result<Self, PlatformError> {
        Ok(Self::new())
    }

    /// Lexes `input` into the kit's token stream.
    pub fn lex(&self, input: &str) -> Result<Vec<Token<CanonicalKind>>, DefaultLexError> {
        CanonicalLexer.lex(input)
    }

    /// Parses `input` into a generic [`KitNode`].
    pub fn parse(&self, input: &str) -> Result<KitNode, ParseError> {
        let tokens = self.lex(input).map_err(|e| ParseError::LexError {
            message: e.to_string(),
        })?;

        let grammar = CompositeGrammar::new()
            .register(NumberRule)
            .register(StringRule)
            .register(BoolRule)
            .register(NullRule)
            .register(IdentifierRule)
            .register(ArrayRule)
            .register(ObjectRule)
            .register(LetRule)
            .register(MatchRule)
            .register(HandleRule)
            .register(CallRule)
            .register(IndexRule)
            .register(PipeRule)
            .register(AssignRule)
            .eof(CanonicalKind::Eof);

        KitParser::new(grammar).parse(tokens)
    }

    /// Parses a module. Not yet supported by the minimal canonical plugin set.
    pub fn parse_module(&self, _input: &str) -> Result<Vec<KitNode>, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// ADR-0002 `Library` plugin for the canonical kit.
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
        "kit.canonical"
    }

    fn name(&self) -> &str {
        "kit-canonical"
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
        ctx.register_plugin(BasicParser::new())?;
        ctx.register_plugin(BasicFormatter::new())?;
        ctx.register_plugin(BasicLinter::new())?;
        ctx.register_plugin(BasicTypechecker::new())?;
        ctx.register_plugin(BasicEvaluator::new())?;
        ctx.register_plugin(BasicBuilder::new())?;
        ctx.register_plugin(BasicCompiler::new())?;

        // Format and lint behavior contributors (ADR-0034 P1.3)
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
        assert_eq!(kit.parse("1").unwrap(), KitNode::Literal(KitValue::Number(1.0)));
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
        let expected = KitNode::Pipe(Box::new(Pipe::new(KitNode::Literal(KitValue::Number(1.0)), right)));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_let() {
        let kit = Kit::new();
        let node = kit.parse("let a = 1;").unwrap();
        let expected = KitNode::Let(Box::new(Let::new(
            "a".into(),
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
        assert_eq!(kit.parse("true").unwrap(), KitNode::Literal(KitValue::Bool(true)));
        assert_eq!(kit.parse("false").unwrap(), KitNode::Literal(KitValue::Bool(false)));
    }

    #[test]
    fn parse_null_literal() {
        let kit = Kit::new();
        assert_eq!(kit.parse("null").unwrap(), KitNode::Literal(KitValue::Null));
    }

    #[test]
    fn parse_module_unsupported() {
        let kit = Kit::new();
        assert!(matches!(kit.parse_module(""), Err(PlatformError::NotImplemented)));
    }
}
