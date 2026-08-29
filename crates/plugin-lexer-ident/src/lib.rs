//! [`IdentRule`] — matches identifiers and keywords.

use handl_plugin_lexer::{DefaultLexError, IdentFactory, KindFactory, LexError, LexerConfig, LexRule, Token, TokenKind};

/// Matches identifiers and keywords (identifiers in the keyword table
/// are remapped to their token kind).
#[derive(Clone, Default)]
pub struct IdentRule;

impl<K, E> LexRule<K, E> for IdentRule
where
    K: TokenKind,
    K::Factory: IdentFactory<K>,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        match chars.peek() {
            Some(&(_, ch)) if config.factory().is_ident_start(ch) => Some(
                config
                    .factory()
                    .lex_ident(chars)
                    .map(|(text, span)| {
                        let kind = config
                            .lookup_keyword(&text)
                            .cloned()
                            .unwrap_or_else(|| config.factory().ident_kind(text));
                        Token::new(kind, span)
                    })
                    .map_err(E::from),
            ),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        "ident"
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

    const RULE: IdentRule = IdentRule;

    #[test]
    fn ident_rule_matches_identifier() {
        let config = cfg();
        let mut chars = "foo bar".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Ident("foo".into()));
        assert_eq!(token.span, Span::new(0, 3));
    }

    #[test]
    fn ident_rule_matches_namespace_path() {
        let config = cfg();
        let mut chars = "math.sqrt rest".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Ident("math.sqrt".into()));
    }

    #[test]
    fn ident_rule_keyword_remapped() {
        let config = cfg().with_keyword("true", MockKind::Delim('t'));
        let mut chars = "true rest".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Delim('t'));
    }

    #[test]
    fn ident_rule_no_match_digit() {
        let config = cfg();
        let mut chars = "42".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn ident_rule_no_match_symbol() {
        let config = cfg();
        let mut chars = "@@".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn ident_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "ident");
    }

    #[test]
    fn ident_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "ident");
    }
}
