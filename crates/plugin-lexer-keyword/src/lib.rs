//! [`KeywordRule`] — matches identifiers the factory recognizes as keywords.
//!
//! This crate owns the [`KeywordToken`] trait. Identifier lexing is reused
//! from `plugin-lexer-ident`.

use handl_plugin_lexer::{LexError, LexRule, LexerConfig, Token, TokenKind};
use handl_plugin_lexer_ident::{lex_ident, IdentToken};
use std::iter::Peekable;

// ---------------------------------------------------------------------------
// Token trait
// ---------------------------------------------------------------------------

/// Token-kind factory capability for keywords.
///
/// A host's token kind factory implements this so that [`KeywordRule`] can
/// recognize keyword identifiers. This is a marker extension of [`IdentToken`];
/// the factory must also implement [`IdentToken`] so the rule can lex the
/// identifier text first.
pub trait KeywordToken<K: TokenKind>: IdentToken<K> {
    /// Returns the keyword token kind for `text`, or `None` if not a keyword.
    fn keyword_kind(&self, text: &str) -> Option<K>;
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// Matches identifiers that the factory's [`KeywordToken::keyword_kind`] maps
/// to a keyword kind. Non-keyword identifiers are left for other rules.
#[derive(Clone, Default)]
pub struct KeywordRule;

impl<K, E> LexRule<K, E> for KeywordRule
where
    K: TokenKind,
    K::Factory: KeywordToken<K>,
    E: LexError + From<handl_plugin_lexer::DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        let factory = config.factory();
        match chars.peek() {
            Some(&(_, ch)) if factory.is_ident_start(ch) => {
                let mut peek = chars.clone();
                let (text, span) =
                    lex_ident(&mut peek, |c| factory.is_ident_continue(c)).ok()?;
                factory.keyword_kind(&text).map(|kind| {
                    *chars = peek;
                    Ok(Token::new(kind, span))
                })
            }
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        "keyword"
    }

    fn clone_box(&self) -> Box<dyn LexRule<K, E>> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer::{DefaultLexError, EofFactory, LexRule, LexerConfig, Span, Token, TokenKind};
    use handl_plugin_lexer_ident::IdentToken;
    use std::fmt;

    // Local mock for keyword tests. KeywordToken is local to this crate but
    // requires IdentToken (from plugin-lexer-ident), so we use a local mock
    // factory that implements both.
    #[derive(Debug, Clone, PartialEq)]
    enum MockKind {
        Ident(String),
        Keyword(String),
        Eof,
    }

    impl fmt::Display for MockKind {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                MockKind::Ident(s) => write!(f, "ident({s})"),
                MockKind::Keyword(s) => write!(f, "keyword({s})"),
                MockKind::Eof => write!(f, "eof"),
            }
        }
    }

    impl TokenKind for MockKind {
        type Factory = KeywordMockFactory;
        fn is_eof(&self) -> bool {
            matches!(self, MockKind::Eof)
        }
        fn is_error(&self) -> bool {
            false
        }
    }

    // A configurable factory for keyword tests.
    use std::sync::Arc;

    struct KeywordMockFactory {
        keyword_fn: Arc<dyn Fn(String) -> Option<MockKind> + Send + Sync>,
    }

    impl Default for KeywordMockFactory {
        fn default() -> Self {
            KeywordMockFactory {
                keyword_fn: Arc::new(|_| None),
            }
        }
    }

    impl Clone for KeywordMockFactory {
        fn clone(&self) -> Self {
            KeywordMockFactory {
                keyword_fn: Arc::clone(&self.keyword_fn),
            }
        }
    }

    impl EofFactory<MockKind> for KeywordMockFactory {
        fn eof_kind(&self) -> MockKind {
            MockKind::Eof
        }
    }

    impl IdentToken<MockKind> for KeywordMockFactory {
        fn is_ident_start(&self, ch: char) -> bool {
            ch.is_alphabetic() || ch == '_'
        }

        fn is_ident_continue(&self, ch: char) -> bool {
            ch.is_alphanumeric() || ch == '_'
        }

        fn ident_kind(&self, text: String) -> MockKind {
            MockKind::Ident(text)
        }
    }

    impl KeywordToken<MockKind> for KeywordMockFactory {
        fn keyword_kind(&self, text: &str) -> Option<MockKind> {
            (self.keyword_fn)(text.into())
        }
    }

    impl KeywordMockFactory {
        fn new() -> Self {
            Self::default()
        }

        fn with_keyword_kind<F>(self, f: F) -> Self
        where
            F: Fn(String) -> Option<MockKind> + Send + Sync + 'static,
        {
            KeywordMockFactory {
                keyword_fn: Arc::new(f),
            }
        }
    }

    const RULE: KeywordRule = KeywordRule;

    #[test]
    fn keyword_rule_matches_factory_keyword() {
        let factory = KeywordMockFactory::new().with_keyword_kind(|t| match t.as_str() {
            "await" => Some(MockKind::Keyword(t)),
            _ => None,
        });
        let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(factory);
        let mut chars = "await rest".char_indices().peekable();
        let token = RULE.try_lex(&mut chars, &config).unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Keyword("await".into()));
        assert_eq!(token.span, Span::new(0, 5));
    }

    #[test]
    fn keyword_rule_no_match_non_keyword() {
        let factory = KeywordMockFactory::new().with_keyword_kind(|_| None);
        let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(factory);
        let mut chars = "foo".char_indices().peekable();
        assert!(RULE.try_lex(&mut chars, &config).is_none());
    }

    #[test]
    fn keyword_rule_no_match_digit() {
        let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(KeywordMockFactory::new());
        let mut chars = "42".char_indices().peekable();
        assert!(RULE.try_lex(&mut chars, &config).is_none());
    }

    #[test]
    fn keyword_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "keyword");
    }

    #[test]
    fn keyword_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "keyword");
    }
}
