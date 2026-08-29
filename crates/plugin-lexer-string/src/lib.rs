//! [`StringRule`] — matches string literals.

use handl_plugin_lexer::{DefaultLexError, KindFactory, LexError, LexerConfig, LexRule, StringFactory, Token, TokenKind};

/// Matches string literals using the factory's [`StringFactory::is_string_start`]
/// and [`StringFactory::lex_string`].
#[derive(Clone, Default)]
pub struct StringRule;

impl<K, E> LexRule<K, E> for StringRule
where
    K: TokenKind,
    K::Factory: StringFactory<K>,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        match chars.peek() {
            Some(&(_, ch)) if config.factory().is_string_start(ch) => Some(
                config
                    .factory()
                    .lex_string(chars)
                    .map(|(value, span)| Token::new(config.factory().string_kind(value), span))
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
    use handl_plugin_lexer::{DefaultLexError, LexRule, Span};
    use handl_plugin_lexer_common::{empty_config as cfg, MockKind};

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
