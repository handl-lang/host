//! [`KeywordRule`] — matches identifiers the factory recognizes as keywords.

use handl_plugin_lexer::{
    IdentFactory, KeywordFactory, KindFactory, LexError, LexRule, LexerConfig, Token, TokenKind,
};

/// Matches identifiers that the factory's [`KeywordFactory::keyword_kind`] maps
/// to a keyword kind. Non-keyword identifiers are left for other rules.
#[derive(Clone, Default)]
pub struct KeywordRule;

impl<K, E> LexRule<K, E> for KeywordRule
where
    K: TokenKind,
    K::Factory: KeywordFactory<K> + IdentFactory<K>,
    E: LexError,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        match chars.peek() {
            Some(&(_, ch)) if config.factory().is_ident_start(ch) => {
                let mut peek = chars.clone();
                let (text, span) = config.factory().lex_ident(&mut peek).ok()?;
                config.factory().keyword_kind(&text).map(|kind| {
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
    use handl_plugin_lexer::{DefaultLexError, LexRule, LexerConfig, Span};
    use handl_plugin_lexer_common::{MockFactory, MockKind};

    const RULE: KeywordRule = KeywordRule;

    #[test]
    fn keyword_rule_matches_factory_keyword() {
        let factory = MockFactory::new().with_keyword_kind(|t| match t.as_str() {
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
        let factory = MockFactory::new().with_keyword_kind(|_| None);
        let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(factory);
        let mut chars = "foo".char_indices().peekable();
        assert!(RULE.try_lex(&mut chars, &config).is_none());
    }

    #[test]
    fn keyword_rule_no_match_digit() {
        let config: LexerConfig<MockKind, DefaultLexError> = LexerConfig::new(MockFactory::new());
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
