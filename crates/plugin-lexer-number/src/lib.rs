//! [`NumberRule`] — matches numbers including negative numbers.

use handl_plugin_lexer::{
    DefaultLexError, KindFactory, LexError, LexRule, LexerConfig, NumberFactory, Token, TokenKind,
};

/// Matches number literals using the factory's [`NumberFactory::is_number_start`]
/// and [`NumberFactory::lex_number`].
#[derive(Clone, Default)]
pub struct NumberRule;

impl<K, E> LexRule<K, E> for NumberRule
where
    K: TokenKind,
    K::Factory: NumberFactory<K>,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
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
                    config
                        .factory()
                        .lex_number(chars)
                        .map(|(value, span)| Token::new(config.factory().number_kind(value), span))
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
mod tests {
    use super::*;
    use handl_plugin_lexer::{DefaultLexError, LexRule, Span};
    use handl_plugin_lexer_common::{MockKind, empty_config as cfg};

    const RULE: NumberRule = NumberRule;

    #[test]
    fn number_rule_matches_integer() {
        let config = cfg();
        let mut chars = "42 rest".char_indices().peekable();
        let result = NumberRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Num(42.0));
        assert_eq!(token.span, Span::new(0, 2));
    }

    #[test]
    fn number_rule_matches_float() {
        let config = cfg();
        let mut chars = "1.5 rest".char_indices().peekable();
        let result = NumberRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Num(1.5));
    }

    #[test]
    fn number_rule_matches_negative() {
        let config = cfg();
        let mut chars = "-3 rest".char_indices().peekable();
        let result = NumberRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Num(-3.0));
    }

    #[test]
    fn number_rule_dash_without_digit_no_match() {
        let config = cfg();
        let mut chars = "- rest".char_indices().peekable();
        let result = NumberRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn number_rule_no_match() {
        let config = cfg();
        let mut chars = "hello".char_indices().peekable();
        let result = NumberRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn number_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "number");
    }

    #[test]
    fn number_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "number");
    }
}
