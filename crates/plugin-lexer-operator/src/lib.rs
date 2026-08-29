//! [`OperatorRule`] — matches operators.

use handl_plugin_lexer::{LexError, LexerConfig, LexRule, OperatorFactory, Token, TokenKind};

/// Matches operators using the factory's [`OperatorFactory::lex_operator`].
#[derive(Clone, Default)]
pub struct OperatorRule;

impl<K, E> LexRule<K, E> for OperatorRule
where
    K: TokenKind,
    K::Factory: OperatorFactory<K>,
    E: LexError,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        config
            .factory()
            .lex_operator(chars)
            .map(|(kind, span)| Ok(Token::new(kind, span)))
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
    use handl_plugin_lexer::{DefaultLexError, LexRule, LexerConfig, Span};
    use handl_plugin_lexer_common::{MockFactory, MockKind};

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
