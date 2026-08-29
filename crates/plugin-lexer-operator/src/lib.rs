//! [`OperatorRule`] — matches operators.
//!
//! This crate owns the operator lexing logic (`lex_operator`) and the
//! [`OperatorToken`] trait that a host's token kind factory implements to
//! produce operator token kinds.

use handl_plugin_lexer::{LexError, LexRule, LexerConfig, Span, Token, TokenKind};
use std::iter::Peekable;

// ---------------------------------------------------------------------------
// Primitive
// ---------------------------------------------------------------------------

/// Tries to lex a run of operator characters from `operator_chars`.
/// Returns the raw operator text and its span, or `None` if no operator starts here.
pub fn lex_operator(
    chars: &mut Peekable<std::str::CharIndices<'_>>,
    operator_chars: &str,
) -> Option<(String, Span)> {
    let &(start, first) = chars.peek()?;
    if !operator_chars.contains(first) {
        return None;
    }

    let mut text = String::new();
    let mut end = start;
    while let Some(&(pos, ch)) = chars.peek() {
        if !operator_chars.contains(ch) {
            break;
        }
        text.push(ch);
        end = pos + ch.len_utf8();
        chars.next();
    }

    if text.is_empty() {
        return None;
    }

    Some((text, Span::new(start, end)))
}

// ---------------------------------------------------------------------------
// Token trait
// ---------------------------------------------------------------------------

/// Token-kind factory capability for operators.
///
/// A host's token kind factory implements this so that [`OperatorRule`] can
/// produce operator tokens. The host decides which characters are operator
/// characters and how to map operator text to a token kind.
pub trait OperatorToken<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns the set of characters that can form an operator.
    fn operator_chars(&self) -> &str;

    /// Returns the operator token kind for `text`, or `None` if unrecognized.
    fn operator_kind(&self, text: &str) -> Option<K>;
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// Matches operators using [`lex_operator`] and [`OperatorToken`].
#[derive(Clone, Default)]
pub struct OperatorRule;

impl<K, E> LexRule<K, E> for OperatorRule
where
    K: TokenKind,
    K::Factory: OperatorToken<K>,
    E: LexError,
{
    fn try_lex(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        let factory = config.factory();
        let (text, span) = lex_operator(chars, factory.operator_chars())?;
        factory
            .operator_kind(&text)
            .map(|kind| Ok(Token::new(kind, span)))
    }

    fn name(&self) -> &'static str {
        "operator"
    }

    fn clone_box(&self) -> Box<dyn LexRule<K, E>> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer::{DefaultLexError, LexRule, LexerConfig, Span, TokenKind};
    use handl_plugin_lexer_common::{MockFactory, MockKind};

    const MOCK_OPERATOR_CHARS: &str = "+-*/=<>&|%!^~?";

    // Orphan-rule-allowed impl: OperatorToken is local to this crate.
    impl OperatorToken<MockKind> for MockFactory {
        fn operator_chars(&self) -> &str {
            MOCK_OPERATOR_CHARS
        }

        fn operator_kind(&self, text: &str) -> Option<MockKind> {
            (self.operator_kind_fn())(text.into())
        }
    }

    const RULE: OperatorRule = OperatorRule;

    #[test]
    fn operator_rule_matches_factory_operator() {
        let factory = MockFactory::new().with_operator_kind(|t| match t.as_str() {
            "+" => Some(MockKind::Operator(t)),
            _ => None,
        });
        let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(factory);
        let mut chars = "+ rest".char_indices().peekable();
        let token = RULE.try_lex(&mut chars, &config).unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Operator("+".into()));
        assert_eq!(token.span, Span::new(0, 1));
    }

    #[test]
    fn operator_rule_no_match_unconfigured() {
        let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(MockFactory::new());
        let mut chars = "+".char_indices().peekable();
        assert!(RULE.try_lex(&mut chars, &config).is_none());
    }

    #[test]
    fn operator_rule_no_match_letter() {
        let factory = MockFactory::new().with_operator_kind(|_| None);
        let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(factory);
        let mut chars = "hello".char_indices().peekable();
        assert!(RULE.try_lex(&mut chars, &config).is_none());
    }

    #[test]
    fn operator_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "operator");
    }

    #[test]
    fn operator_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "operator");
    }
}
