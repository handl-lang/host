//! [`NumberRule`] — matches numbers including negative numbers.
//!
//! This crate owns the number lexing logic (`lex_number`) and the
//! [`NumberToken`] trait that a host's token kind factory implements to
//! produce number token kinds.

use handl_plugin_lexer::{
    DefaultLexError, LexError, LexRule, LexerConfig, Span, Token, TokenKind,
};
use std::iter::Peekable;

// ---------------------------------------------------------------------------
// Primitive
// ---------------------------------------------------------------------------

/// Lexes a decimal number literal with an optional leading `-`.
/// Returns the parsed `f64` value and its span.
pub fn lex_number(
    chars: &mut Peekable<std::str::CharIndices<'_>>,
) -> Result<(f64, Span), DefaultLexError> {
    let (start, _) = chars.peek().copied().unwrap();
    let mut text = String::new();
    let mut end = start;

    if chars.peek().map(|(_, c)| *c) == Some('-') {
        let (pos, ch) = chars.next().unwrap();
        text.push(ch);
        end = pos + 1;
    }

    while let Some(&(pos, ch)) = chars.peek() {
        if ch.is_ascii_digit() || ch == '.' {
            text.push(ch);
            end = pos + ch.len_utf8();
            chars.next();
        } else {
            break;
        }
    }

    let value: f64 = text.parse().map_err(|_| DefaultLexError::InvalidNumber {
        span: Span::new(start, end),
    })?;

    Ok((value, Span::new(start, end)))
}

// ---------------------------------------------------------------------------
// Token trait
// ---------------------------------------------------------------------------

/// Token-kind factory capability for number literals.
///
/// A host's token kind factory implements this so that [`NumberRule`] can
/// produce number tokens. The host decides which characters start a number
/// and how to turn an `f64` into a token kind.
pub trait NumberToken<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns `true` if `ch` can start a number literal.
    fn is_number_start(&self, ch: char) -> bool;

    /// Builds a number token kind from the parsed `f64` value.
    fn number_kind(&self, value: f64) -> K;

    /// Returns a regex pattern that matches number literals for this dialect,
    /// e.g. `r"-?\d+(\.\d+)?([eE][+-]?\d+)?"`.
    ///
    /// Override this to provide a dialect-specific number regex for grammar
    /// generators. The default returns a basic integer/decimal pattern.
    fn number_regex(&self) -> &str {
        r"-?\d+(\.\d+)?([eE][+-]?\d+)?"
    }
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// Matches number literals using the factory's [`NumberToken::is_number_start`]
/// and [`lex_number`].
#[derive(Clone, Default)]
pub struct NumberRule;

impl<K, E> LexRule<K, E> for NumberRule
where
    K: TokenKind,
    K::Factory: NumberToken<K>,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        match chars.peek() {
            Some(&(_, ch)) if config.factory().is_number_start(ch) => {
                if ch == '-' {
                    let mut peek = chars.clone();
                    peek.next();
                    if !matches!(peek.peek(), Some((_, c)) if c.is_ascii_digit()) {
                        return None;
                    }
                }
                Some(
                    lex_number(chars)
                        .map(|(value, span)| {
                            Token::new(config.factory().number_kind(value), span)
                        })
                        .map_err(E::from),
                )
            }
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        "number"
    }

    fn clone_box(&self) -> Box<dyn LexRule<K, E>> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod lib_tests;
