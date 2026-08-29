//! Token factories for dialect-specific token construction.
//!
//! Each factory is both a constructor and, where relevant, a primitive
//! lexer. [`KindFactory`] is a convenience trait that rolls the common
//! token kind constructors into one. The sub-traits ([`StringFactory`],
//! [`NumberFactory`], [`IdentFactory`], [`KeywordFactory`],
//! [`OperatorFactory`], [`CommentFactory`]) add dialect-specific
//! tokenization behaviour; each concrete factory decides which prefixes,
//! character sets, and literal syntax to use.

use crate::{DefaultLexError, Span, TokenKind};
use std::iter::Peekable;

/// Factory for producing the end-of-input token.
pub trait EofFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns the EOF token kind for the dialect.
    fn eof_kind(&self) -> K;
}

/// Convenience trait that rolls the common token kind constructors into one.
pub trait KindFactory<K: TokenKind>: EofFactory<K> + Clone + Send + Sync + 'static {
    /// Builds a string token kind.
    fn string_kind(&self, value: String) -> K;

    /// Builds a number token kind.
    fn number_kind(&self, value: f64) -> K;

    /// Builds an identifier token kind.
    fn ident_kind(&self, text: String) -> K;

    /// Returns the keyword token kind for `text`, or `None`.
    fn keyword_kind(&self, text: &str) -> Option<K>;

    /// Returns the operator token kind for `text`, or `None`.
    fn operator_kind(&self, text: &str) -> Option<K>;
}

/// Factory for string literals.
pub trait StringFactory<K: TokenKind>: KindFactory<K> {
    /// Returns `true` if `ch` can start a string literal.
    fn is_string_start(&self, ch: char) -> bool;

    /// Lexes a string literal and returns its unescaped value and span.
    fn lex_string(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError>;
}

/// Factory for number literals.
pub trait NumberFactory<K: TokenKind>: KindFactory<K> {
    /// Returns `true` if `ch` can start a number literal.
    fn is_number_start(&self, ch: char) -> bool;

    /// Lexes a number literal and returns its `f64` value and span.
    fn lex_number(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(f64, Span), DefaultLexError>;
}

/// Factory for identifiers and keywords.
pub trait IdentFactory<K: TokenKind>: KindFactory<K> {
    /// Returns `true` if `ch` can start an identifier.
    fn is_ident_start(&self, ch: char) -> bool;

    /// Returns `true` if `ch` may appear after the first char of an identifier.
    fn is_ident_continue(&self, ch: char) -> bool;

    /// Lexes an identifier and returns its raw text and span.
    fn lex_ident(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError>;
}

/// Marker trait for factories that can map identifier text to keyword kinds.
pub trait KeywordFactory<K: TokenKind>: IdentFactory<K> {}

/// Factory for operator tokens.
pub trait OperatorFactory<K: TokenKind>: KindFactory<K> {
    /// Tries to lex an operator at the current position.
    fn lex_operator(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> Option<(K, Span)>;
}

/// Factory for comment tokens.
pub trait CommentFactory<K: TokenKind>: KindFactory<K> {
    /// Builds a comment token kind from the comment text.
    fn comment_kind(&self, text: String) -> K;

    /// Builds a doc comment token kind from the doc comment text.
    fn doc_comment_kind(&self, text: String) -> K;

    /// Returns the line comment prefix string, if any (e.g. `"//"`).
    ///
    /// Override this to enable grammar generators to introspect comment styles
    /// without peeking into the lexing logic.
    fn line_comment_prefix(&self) -> Option<&str> {
        None
    }

    /// Returns the block comment (start, end) strings, if any (e.g. `("/*", "*/")`).
    fn block_comment_delims(&self) -> Option<(&str, &str)> {
        None
    }

    /// Returns the doc line comment prefix string, if any (e.g. `"///"`).
    fn doc_line_comment_prefix(&self) -> Option<&str> {
        None
    }

    /// Returns the doc block comment (start, end) strings, if any (e.g. `("/**", "*/")`).
    fn doc_block_comment_delims(&self) -> Option<(&str, &str)> {
        None
    }

    /// Returns `true` if the iterator is at the start of a line comment.
    fn is_line_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool;

    /// Lexes a line comment and returns its text and span.
    fn lex_line_comment(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> (String, Span);

    /// Returns `true` if the iterator is at the start of a block comment.
    fn is_block_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool;

    /// Lexes a block comment and returns its text and span, or an error if unterminated.
    fn lex_block_comment(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError>;

    /// Returns `true` if the iterator is at the start of a doc line comment.
    fn is_doc_line_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool;

    /// Lexes a doc line comment and returns its text and span.
    fn lex_doc_line_comment(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> (String, Span);

    /// Returns `true` if the iterator is at the start of a doc block comment.
    fn is_doc_block_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool;

    /// Lexes a doc block comment and returns its text and span, or an error if unterminated.
    fn lex_doc_block_comment(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError>;
}
