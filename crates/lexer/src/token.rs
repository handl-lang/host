use crate::Span;
use std::fmt;

/// A token kind — the type of a lexed token.
///
/// Each concrete token kind or a dialect's token-kind enum implements
/// this trait. The base crate is deliberately agnostic about what the
/// concrete kinds are: numbers, strings, identifiers, comments, and so on
/// can each be provided by separate `plugin-token-*` crates.
///
/// # Implementing
///
/// ```
/// use handl_lexer::TokenKind;
/// use std::fmt;
///
/// #[derive(Debug, Clone, PartialEq)]
/// enum MyKind {
///     Identifier(String),
///     Number(f64),
///     Eof,
/// }
///
/// impl fmt::Display for MyKind {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         match self {
///             MyKind::Identifier(s) => write!(f, "identifier({s})"),
///             MyKind::Number(n) => write!(f, "number({n})"),
///             MyKind::Eof => write!(f, "eof"),
///         }
///     }
/// }
///
/// impl TokenKind for MyKind {
///     fn is_eof(&self) -> bool { matches!(self, MyKind::Eof) }
///     fn is_error(&self) -> bool { false }
/// }
/// ```
pub trait TokenKind: Clone + fmt::Debug + fmt::Display + PartialEq + Send + Sync + 'static {
    /// The factory that constructs tokens for this dialect.
    type Factory: crate::EofFactory<Self> + Clone + Send + Sync + 'static;

    /// Returns `true` if this is the end-of-input sentinel token.
    fn is_eof(&self) -> bool;

    /// Returns `true` if this is a recoverable error token.
    fn is_error(&self) -> bool;
}

/// A token kind that can represent comments.
///
/// Dialects with comment tokens implement this trait. This is an optional
/// capability; lexers that need to filter comments can require it.
pub trait CommentTokenKind: TokenKind {
    /// Returns `true` if this is a comment token (to be filtered by the lexer).
    fn is_comment(&self) -> bool;
}

/// A lexed token — its kind and source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Token<K: TokenKind> {
    /// What kind of token this is.
    pub kind: K,

    /// Where this token appears in the source text.
    pub span: Span,
}

impl<K: TokenKind> Token<K> {
    /// Creates a token at the given span.
    pub fn new(kind: K, span: Span) -> Self {
        Token { kind, span }
    }
}
