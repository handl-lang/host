//! Dynamic token kind for the handl dialect.
//!
//! [`DynKind`] wraps `Box<dyn DynTokenKind>` and implements [`TokenKind`],
//! making it a first-class token kind for the generic lexer and parser.
//! Each per-concern kind (`NumberKind`, `StringKind`, `AsKind`, etc.) is
//! boxed into `DynKind` by [`DynFactory`]. Adding a new token kind = adding
//! a new `plugin-token-*` crate and a line in `DynFactory` — no central enum
//! to edit.
//!
//! This crate also provides [`DynLexer`] (the concrete lexer) and the
//! [`DynFactory`] that wires all per-concern capability traits together.

use std::any::Any;
use std::fmt;

use handl_plugin_lexer::{
    CommentTokenKind, DefaultLexError, DynTokenKind, EofFactory, EofKind, Lexer, LexerConfig,
    Token, TokenKind,
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

// Per-concern token traits
use handl_plugin_lexer_ident::IdentToken;
use handl_plugin_lexer_keyword::KeywordToken;
use handl_plugin_lexer_line_comment::CommentToken;
use handl_plugin_lexer_number::NumberToken;
use handl_plugin_lexer_operator::OperatorToken;
use handl_plugin_lexer_string::StringToken;

// Per-concern kinds
use handl_plugin_token_bool::{BoolKind, TokenFactory as BoolFactory};
use handl_plugin_token_comment::{CommentKind, TokenFactory as CommentFactory};
use handl_plugin_token_delimiter::{DelimiterKind, TokenFactory as DelimiterFactory};
use handl_plugin_token_doc_comment::{DocCommentKind, DocCommentRule, TokenFactory as DocCommentFactory};
use handl_plugin_token_identifier::{IdentKind, TokenFactory as IdentFactory};
use handl_plugin_token_keyword_as::{AsKind, TokenFactory as AsFactory};
use handl_plugin_token_keyword_handle::{HandleKind, TokenFactory as HandleFactory};
use handl_plugin_token_keyword::{KeywordKind, TokenFactory as KeywordFactory};
use handl_plugin_token_null::{NullKind, TokenFactory as NullFactory};
use handl_plugin_token_number::{NumberKind, TokenFactory as NumberFactory};
use handl_plugin_token_operator::{CANONICAL_OPERATOR_CHARS, OperatorKind, TokenFactory as OperatorFactory};
use handl_plugin_token_string::{StringKind, TokenFactory as StringFactory};

use handl_plugin_grammar_textmate::{GrammarContributor, GrammarRule};

pub use handl_plugin_lexer_template::TemplateSegment as DynTemplateSegment;

// ---------------------------------------------------------------------------
// TemplateKind — token kind for template literals
// ---------------------------------------------------------------------------

/// Token kind for template string literals.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateKind(pub Vec<TemplateSegment>);

impl fmt::Display for TemplateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "template(")?;
        for seg in &self.0 {
            match seg {
                TemplateSegment::Text(t) => write!(f, "{t:?}")?,
                TemplateSegment::Expr(e) => write!(f, "${{{e}}}")?,
            }
        }
        write!(f, ")")
    }
}

impl DynTokenKind for TemplateKind {
    fn is_eof(&self) -> bool {
        false
    }

    fn is_error(&self) -> bool {
        false
    }

    fn eq_kind(&self, other: &dyn DynTokenKind) -> bool {
        other
            .as_any()
            .downcast_ref::<TemplateKind>()
            .map(|k| k.0 == self.0)
            .unwrap_or(false)
    }

    fn dyn_clone(&self) -> Box<dyn DynTokenKind> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// DynKind — dynamic, open token kind
// ---------------------------------------------------------------------------

/// Dynamic token kind wrapping `Box<dyn DynTokenKind>`.
///
/// This is the canonical token kind for the handl dialect. It is open:
/// any type implementing [`DynTokenKind`] can be boxed into `DynKind` by
/// [`DynFactory`]. Consumers query via [`DynKind::downcast_ref`].
#[derive(Debug)]
pub struct DynKind(pub Box<dyn DynTokenKind>);

impl Clone for DynKind {
    fn clone(&self) -> Self {
        DynKind(self.0.dyn_clone())
    }
}

impl DynKind {
    /// Creates a `DynKind` from any `DynTokenKind`.
    pub fn new(kind: impl DynTokenKind) -> Self {
        DynKind(Box::new(kind))
    }

    /// Downcasts the inner kind to `T`.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref::<T>()
    }

    /// Returns the inner trait object.
    pub fn inner(&self) -> &dyn DynTokenKind {
        self.0.as_ref()
    }
}

impl PartialEq for DynKind {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_kind(other.0.as_ref())
    }
}

impl fmt::Display for DynKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TokenKind for DynKind {
    type Factory = DynFactory;

    fn is_eof(&self) -> bool {
        self.0.is_eof()
    }

    fn is_error(&self) -> bool {
        self.0.is_error()
    }
}

impl CommentTokenKind for DynKind {
    fn is_comment(&self) -> bool {
        self.downcast_ref::<CommentKind>().is_some()
            || self.downcast_ref::<DocCommentKind>().is_some()
    }
}

impl TemplateTokenKind for DynKind {
    fn template_kind(segments: Vec<TemplateSegment>) -> Self {
        DynKind(Box::new(TemplateKind(segments)))
    }
}

impl SemicolonToken for DynKind {
    fn is_semicolon(&self) -> bool {
        self.downcast_ref::<DelimiterKind>()
            .map(|k| *k == DelimiterKind::Semicolon)
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// DynFactory — wires all per-concern capability traits for DynKind
// ---------------------------------------------------------------------------

/// Factory that produces `DynKind` tokens for the canonical handl dialect.
///
/// Each capability method boxes the appropriate per-concern kind. Adding a
/// new token kind = adding a new method mapping here and a dependency on the
/// new `plugin-token-*` crate.
#[derive(Debug, Default, Clone, Copy)]
pub struct DynFactory;

impl EofFactory<DynKind> for DynFactory {
    fn eof_kind(&self) -> DynKind {
        DynKind(Box::new(EofKind))
    }
}

impl StringToken<DynKind> for DynFactory {
    fn is_string_start(&self, ch: char) -> bool {
        ch == '"' || ch == '\''
    }

    fn string_kind(&self, value: String) -> DynKind {
        DynKind(Box::new(StringKind::String(value)))
    }
}

impl NumberToken<DynKind> for DynFactory {
    fn is_number_start(&self, ch: char) -> bool {
        ch.is_ascii_digit() || ch == '-'
    }

    fn number_kind(&self, value: f64) -> DynKind {
        DynKind(Box::new(NumberKind::Number(value)))
    }

    fn number_regex(&self) -> &str {
        r"-?\d+(\.\d+)?([eE][+-]?\d+)?"
    }
}

impl IdentToken<DynKind> for DynFactory {
    fn is_ident_start(&self, ch: char) -> bool {
        ch.is_alphabetic() || ch == '_'
    }

    fn is_ident_continue(&self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    fn ident_kind(&self, text: String) -> DynKind {
        DynKind(Box::new(IdentKind::Ident(text)))
    }

    fn identifier_regex(&self) -> &str {
        r"[a-zA-Z_.][a-zA-Z0-9_.]*"
    }
}

impl KeywordToken<DynKind> for DynFactory {
    fn keyword_kind(&self, text: &str) -> Option<DynKind> {
        match text {
            "true" => Some(DynKind(Box::new(BoolKind::Bool(true)))),
            "false" => Some(DynKind(Box::new(BoolKind::Bool(false)))),
            "null" => Some(DynKind(Box::new(NullKind::Null))),
            "as" => Some(DynKind(Box::new(AsKind::As))),
            "handle" => Some(DynKind(Box::new(HandleKind::Handle))),
            "match" | "typeof" | "not" | "import" | "export" | "module" => {
                Some(DynKind(Box::new(KeywordKind(
                    // SAFETY: all arms are 'static strings
                    match text {
                        "match" => "match",
                        "typeof" => "typeof",
                        "not" => "not",
                        "import" => "import",
                        "export" => "export",
                        "module" => "module",
                        _ => unreachable!(),
                    },
                ))))
            }
            _ => None,
        }
    }
}

impl OperatorToken<DynKind> for DynFactory {
    fn operator_chars(&self) -> &str {
        CANONICAL_OPERATOR_CHARS
    }

    fn operator_kind(&self, text: &str) -> Option<DynKind> {
        let op = match text {
            "|>" => OperatorKind::Pipe,
            "->" => OperatorKind::Arrow,
            "=" => OperatorKind::Equal,
            "+" => OperatorKind::Plus,
            "-" => OperatorKind::Minus,
            "*" => OperatorKind::Star,
            "/" => OperatorKind::Slash,
            "<" => OperatorKind::LessThan,
            ">" => OperatorKind::GreaterThan,
            "==" => OperatorKind::EqualEqual,
            "!=" => OperatorKind::NotEqual,
            "<=" => OperatorKind::LessEqual,
            ">=" => OperatorKind::GreaterEqual,
            "<=>" => OperatorKind::Spaceship,
            _ => return None,
        };
        Some(DynKind(Box::new(op)))
    }
}

impl CommentToken<DynKind> for DynFactory {
    fn comment_kind(&self, text: String) -> DynKind {
        DynKind(Box::new(CommentKind::Comment(text)))
    }

    fn doc_comment_kind(&self, text: String) -> DynKind {
        DynKind(Box::new(DocCommentKind::DocComment(text)))
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
}

// ---------------------------------------------------------------------------
// DynLexer — concrete lexer for the handl dialect
// ---------------------------------------------------------------------------

/// The concrete lexer for the handl dialect.
#[derive(Debug, Default, Clone, Copy)]
pub struct DynLexer;

impl DynLexer {
    /// Returns the lexer configuration used by this lexer.
    ///
    /// Exposed so tooling (grammar generators, semantic token providers) can
    /// introspect keywords, delimiters, and rules without duplicating them.
    pub fn config() -> LexerConfig<DynKind, DefaultLexError> {
        LexerConfig::new(DynFactory)
            .with_delimiter('(', DynKind(Box::new(DelimiterKind::ParenOpen)))
            .with_delimiter(')', DynKind(Box::new(DelimiterKind::ParenClose)))
            .with_delimiter('[', DynKind(Box::new(DelimiterKind::BracketOpen)))
            .with_delimiter(']', DynKind(Box::new(DelimiterKind::BracketClose)))
            .with_delimiter('{', DynKind(Box::new(DelimiterKind::BraceOpen)))
            .with_delimiter('}', DynKind(Box::new(DelimiterKind::BraceClose)))
            .with_delimiter(',', DynKind(Box::new(DelimiterKind::Comma)))
            .with_delimiter(':', DynKind(Box::new(DelimiterKind::Colon)))
            .with_delimiter('.', DynKind(Box::new(DelimiterKind::Dot)))
            .with_delimiter(';', DynKind(Box::new(DelimiterKind::Semicolon)))
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

    /// Returns all keyword text and their kinds, as the single source of truth.
    pub fn all_keywords() -> Vec<(&'static str, DynKind)> {
        vec![
            ("true", DynKind(Box::new(BoolKind::Bool(true)))),
            ("false", DynKind(Box::new(BoolKind::Bool(false)))),
            ("null", DynKind(Box::new(NullKind::Null))),
            ("match", DynKind(Box::new(KeywordKind("match")))),
            ("handle", DynKind(Box::new(HandleKind::Handle))),
            ("as", DynKind(Box::new(AsKind::As))),
            ("typeof", DynKind(Box::new(KeywordKind("typeof")))),
            ("not", DynKind(Box::new(KeywordKind("not")))),
            ("import", DynKind(Box::new(KeywordKind("import")))),
            ("export", DynKind(Box::new(KeywordKind("export")))),
            ("module", DynKind(Box::new(KeywordKind("module")))),
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

impl Lexer<DynKind, DefaultLexError> for DynLexer {
    fn lex(&self, input: &str) -> Result<Vec<Token<DynKind>>, DefaultLexError> {
        let tokens = Self::config().lex(input)?;

        Ok(tokens
            .into_iter()
            .filter(|t| !t.kind.is_comment())
            .collect())
    }
}

// ---------------------------------------------------------------------------
// GrammarContributor
// ---------------------------------------------------------------------------

impl GrammarContributor for DynFactory {
    fn grammar_rules(&self) -> Vec<GrammarRule> {
        let contributors: [&dyn GrammarContributor; 12] = [
            &DocCommentFactory,
            &CommentFactory,
            &StringFactory,
            &NumberFactory,
            &BoolFactory,
            &NullFactory,
            &KeywordFactory,
            &AsFactory,
            &HandleFactory,
            &OperatorFactory,
            &DelimiterFactory,
            &IdentFactory,
        ];

        handl_plugin_grammar_textmate::collect_rules(&contributors)
    }
}
