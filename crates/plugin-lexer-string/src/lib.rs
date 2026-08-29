//! [`StringRule`] — matches string literals.
//!
//! This crate owns the string lexing logic (`lex_string`) and the
//! [`StringToken`] trait that a host's token kind factory implements to
//! produce string token kinds.

use handl_plugin_lexer::{
    DefaultLexError, LexError, LexRule, LexerConfig, Span, Token, TokenKind,
};
use std::iter::Peekable;

// ---------------------------------------------------------------------------
// Primitive
// ---------------------------------------------------------------------------

/// Lexes a string literal. The first character is used as the quote,
/// and the literal runs until the matching quote is found.
pub fn lex_string(
    chars: &mut Peekable<std::str::CharIndices<'_>>,
) -> Result<(String, Span), DefaultLexError> {
    let (start, quote) = chars.next().unwrap();
    let mut value = String::new();
    let mut end = start + 1;

    loop {
        match chars.next() {
            None => {
                return Err(DefaultLexError::UnterminatedString {
                    span: Span::new(start, end),
                });
            }
            Some((pos, ch)) if ch == quote => {
                end = pos + 1;
                break;
            }
            Some((_, '\\')) => match chars.next() {
                Some((_, 'n')) => value.push('\n'),
                Some((_, 't')) => value.push('\t'),
                Some((_, 'r')) => value.push('\r'),
                Some((_, '\\')) => value.push('\\'),
                Some((_, '"')) => value.push('"'),
                Some((_, '\'')) => value.push('\''),
                Some((pos, ch)) => {
                    value.push(ch);
                    let _ = pos;
                }
                None => {
                    return Err(DefaultLexError::UnterminatedString {
                        span: Span::new(start, end),
                    });
                }
            },
            Some((_, ch)) => {
                value.push(ch);
            }
        }
    }

    Ok((value, Span::new(start, end)))
}

// ---------------------------------------------------------------------------
// Token trait
// ---------------------------------------------------------------------------

/// Token-kind factory capability for string literals.
///
/// A host's token kind factory implements this so that [`StringRule`] can
/// produce string tokens. The host decides which characters start a string.
pub trait StringToken<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns `true` if `ch` can start a string literal.
    fn is_string_start(&self, ch: char) -> bool;

    /// Builds a string token kind from the unescaped value.
    fn string_kind(&self, value: String) -> K;
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// Matches string literals using the factory's [`StringToken::is_string_start`]
/// and [`lex_string`].
#[derive(Clone, Default)]
pub struct StringRule;

impl<K, E> LexRule<K, E> for StringRule
where
    K: TokenKind,
    K::Factory: StringToken<K>,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        match chars.peek() {
            Some(&(_, ch)) if config.factory().is_string_start(ch) => Some(
                lex_string(chars)
                    .map(|(value, span)| {
                        Token::new(config.factory().string_kind(value), span)
                    })
                    .map_err(E::from),
            ),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        "string"
    }

    fn clone_box(&self) -> Box<dyn LexRule<K, E>> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer::{DefaultLexError, LexRule, Span, TokenKind};
    use handl_plugin_lexer_common::{MockFactory, MockKind, empty_config as cfg};

    // Orphan-rule-allowed impl: StringToken is local to this crate.
    impl StringToken<MockKind> for MockFactory {
        fn is_string_start(&self, ch: char) -> bool {
            ch == '"' || ch == '\''
        }

        fn string_kind(&self, value: String) -> MockKind {
            (self.string_kind_fn())(value)
        }
    }

    const RULE: StringRule = StringRule;

    #[test]
    fn string_rule_matches_double_quote() {
        let config = cfg();
        let mut chars = "\"hello\" rest".char_indices().peekable();
        let result = StringRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Str("hello".into()));
        assert_eq!(token.span, Span::new(0, 7));
    }

    #[test]
    fn string_rule_matches_single_quote() {
        let config = cfg();
        let mut chars = "'world' rest".char_indices().peekable();
        let result = StringRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Str("world".into()));
    }

    #[test]
    fn string_rule_no_match() {
        let config = cfg();
        let mut chars = "hello".char_indices().peekable();
        let result = StringRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn string_rule_unterminated_errors() {
        let config = cfg();
        let mut chars = "\"oops".char_indices().peekable();
        let result = StringRule.try_lex(&mut chars, &config);
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn string_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "string");
    }

    #[test]
    fn string_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "string");
    }
}
