use crate::Span;
use std::any::Any;
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
/// use handl_plugin_lexer::{EofFactory, TokenKind};
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
///     type Factory = ();
///     fn is_eof(&self) -> bool { matches!(self, MyKind::Eof) }
///     fn is_error(&self) -> bool { false }
/// }
///
/// impl EofFactory<MyKind> for () {
///     fn eof_kind(&self) -> MyKind { MyKind::Eof }
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

/// Object-safe token kind trait for dynamic composition.
pub trait DynTokenKind: fmt::Debug + fmt::Display + Send + Sync + 'static {
    fn is_eof(&self) -> bool;
    fn is_error(&self) -> bool;
    fn eq_kind(&self, other: &dyn DynTokenKind) -> bool;
    fn dyn_clone(&self) -> Box<dyn DynTokenKind>;
    fn as_any(&self) -> &dyn Any;
}

impl<T> DynTokenKind for T
where
    T: TokenKind + Clone + 'static,
{
    fn is_eof(&self) -> bool {
        TokenKind::is_eof(self)
    }

    fn is_error(&self) -> bool {
        TokenKind::is_error(self)
    }

    fn eq_kind(&self, other: &dyn DynTokenKind) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .map_or(false, |o| self == o)
    }

    fn dyn_clone(&self) -> Box<dyn DynTokenKind> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A token with a dynamically-typed kind.
#[derive(Debug)]
pub struct DynToken {
    pub kind: Box<dyn DynTokenKind>,
    pub span: Span,
}

impl DynToken {
    pub fn new(kind: Box<dyn DynTokenKind>, span: Span) -> Self {
        Self { kind, span }
    }
}

impl Clone for DynToken {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.dyn_clone(),
            span: self.span,
        }
    }
}

impl fmt::Display for DynToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{:?}", self.kind, self.span)
    }
}

// ---------------------------------------------------------------------------
// EofKind — canonical end-of-input token kind for dynamic token streams
// ---------------------------------------------------------------------------

/// Canonical end-of-input token kind.
///
/// Used by dynamic token kind factories ([`DynTokenKind`] implementors) to
/// produce the EOF sentinel without requiring each per-concern kind to carry
/// its own `Eof` variant.
#[derive(Debug, Clone, PartialEq)]
pub struct EofKind;

impl fmt::Display for EofKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "eof")
    }
}

impl DynTokenKind for EofKind {
    fn is_eof(&self) -> bool {
        true
    }

    fn is_error(&self) -> bool {
        false
    }

    fn eq_kind(&self, other: &dyn DynTokenKind) -> bool {
        other.as_any().downcast_ref::<EofKind>().is_some()
    }

    fn dyn_clone(&self) -> Box<dyn DynTokenKind> {
        Box::new(EofKind)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
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
