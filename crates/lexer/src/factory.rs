//! Token factories for dialect-specific token construction.
//!
//! Each factory is both a constructor and, where relevant, a primitive
//! lexer. [`KindFactory`] is a convenience trait that rolls the common
//! sub-factories into one; blanket implementations give any [`KindFactory`]
//! implementor the individual [`StringFactory`], [`NumberFactory`],
//! [`IdentFactory`], [`KeywordFactory`], and [`OperatorFactory`] traits.

use crate::{DefaultLexError, Span, TokenKind};
use std::iter::Peekable;

/// Factory for producing the end-of-input token.
pub trait EofFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns the EOF token kind for the dialect.
    fn eof_kind(&self) -> K;
}

/// Factory for string literals.
pub trait StringFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Builds a string token kind from the unescaped value.
    fn string_kind(&self, value: String) -> K;

    /// Returns `true` if `ch` can start a string literal.
    fn is_string_start(&self, ch: char) -> bool {
        crate::primitives::is_string_start(ch)
    }

    /// Lexes a string literal and returns its unescaped value and span.
    fn lex_string(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError> {
        crate::primitives::lex_string(chars)
    }
}

/// Factory for number literals.
pub trait NumberFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Builds a number token kind from the parsed `f64` value.
    fn number_kind(&self, value: f64) -> K;

    /// Returns `true` if `ch` can start a number literal.
    fn is_number_start(&self, ch: char) -> bool {
        crate::primitives::is_number_start(ch)
    }

    /// Lexes a number literal and returns its `f64` value and span.
    fn lex_number(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(f64, Span), DefaultLexError> {
        crate::primitives::lex_number(chars)
    }
}

/// Factory for identifiers and keywords.
pub trait IdentFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Builds an identifier token kind from the raw identifier text.
    fn ident_kind(&self, text: String) -> K;

    /// Returns `true` if `ch` can start an identifier.
    fn is_ident_start(&self, ch: char) -> bool {
        crate::primitives::is_ident_start(ch)
    }

    /// Returns `true` if `ch` can continue an identifier after the first char.
    fn is_ident_continue(&self, ch: char) -> bool {
        crate::primitives::is_ident_continue(ch)
    }

    /// Lexes an identifier and returns its raw text and span.
    fn lex_ident(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError> {
        crate::primitives::lex_ident(chars)
    }
}

/// Factory for keyword tokens.
pub trait KeywordFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns the keyword token kind for `text`, or `None` if it is not a keyword.
    fn keyword_kind(&self, _text: &str) -> Option<K> {
        None
    }
}

/// Factory for operator tokens.
pub trait OperatorFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns the operator token kind for `text`, or `None` if it is not an operator.
    fn operator_kind(&self, _text: &str) -> Option<K> {
        None
    }

    /// Tries to lex an operator at the current position.
    fn lex_operator(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Option<(K, Span)> {
        let &(start, first) = chars.peek()?;
        if !crate::primitives::DEFAULT_OPERATOR_CHARS.contains(first) {
            return None;
        }

        let mut text = String::new();
        let mut end = start;
        while let Some(&(pos, ch)) = chars.peek() {
            if !crate::primitives::DEFAULT_OPERATOR_CHARS.contains(ch) {
                break;
            }
            text.push(ch);
            end = pos + ch.len_utf8();
            chars.next();
        }

        if text.is_empty() {
            return None;
        }

        self.operator_kind(&text).map(|kind| (kind, Span::new(start, end)))
    }
}

/// Factory for comment tokens.
pub trait CommentFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Builds a comment token kind from the comment text.
    fn comment_kind(&self, text: String) -> K;

    /// Returns `true` if the iterator is at the start of a line comment.
    fn is_line_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool {
        crate::primitives::is_line_comment_start(chars)
    }

    /// Lexes a `// line comment` and returns its text and span.
    fn lex_line_comment(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> (String, Span) {
        crate::primitives::lex_line_comment(chars)
    }

    /// Returns `true` if the iterator is at the start of a block comment.
    fn is_block_comment_start(&self, chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool {
        crate::primitives::is_block_comment_start(chars)
    }

    /// Lexes a `/* block comment */` and returns its text and span.
    fn lex_block_comment(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
    ) -> Result<(String, Span), DefaultLexError> {
        crate::primitives::lex_block_comment(chars)
    }
}

/// Convenience trait that rolls the common token factories into one.
pub trait KindFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Builds a string token kind.
    fn string_kind(&self, value: String) -> K;

    /// Builds a number token kind.
    fn number_kind(&self, value: f64) -> K;

    /// Builds an identifier token kind.
    fn ident_kind(&self, text: String) -> K;

    /// Builds an EOF token kind.
    fn eof_kind(&self) -> K;

    /// Returns the keyword token kind for `text`, or `None`.
    fn keyword_kind(&self, _text: &str) -> Option<K> {
        None
    }

    /// Returns the operator token kind for `text`, or `None`.
    fn operator_kind(&self, _text: &str) -> Option<K> {
        None
    }
}

// Blanket implementations: a `KindFactory` is also each sub-factory.
impl<T, K: TokenKind> EofFactory<K> for T
where
    T: KindFactory<K>,
{
    fn eof_kind(&self) -> K {
        KindFactory::eof_kind(self)
    }
}

impl<T, K: TokenKind> StringFactory<K> for T
where
    T: KindFactory<K>,
{
    fn string_kind(&self, value: String) -> K {
        KindFactory::string_kind(self, value)
    }
}

impl<T, K: TokenKind> NumberFactory<K> for T
where
    T: KindFactory<K>,
{
    fn number_kind(&self, value: f64) -> K {
        KindFactory::number_kind(self, value)
    }
}

impl<T, K: TokenKind> IdentFactory<K> for T
where
    T: KindFactory<K>,
{
    fn ident_kind(&self, text: String) -> K {
        KindFactory::ident_kind(self, text)
    }
}

impl<T, K: TokenKind> KeywordFactory<K> for T
where
    T: KindFactory<K>,
{
    fn keyword_kind(&self, text: &str) -> Option<K> {
        KindFactory::keyword_kind(self, text)
    }
}

impl<T, K: TokenKind> OperatorFactory<K> for T
where
    T: KindFactory<K>,
{
    fn operator_kind(&self, text: &str) -> Option<K> {
        KindFactory::operator_kind(self, text)
    }
}
