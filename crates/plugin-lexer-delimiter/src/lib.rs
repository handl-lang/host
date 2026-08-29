//! [`DelimiterRule`] — matches single-character delimiters from config.

use handl_plugin_lexer::{LexError, LexRule, LexerConfig, Span, Token, TokenKind};

/// Matches single-character delimiters from the config's delimiter set.
#[derive(Clone, Default)]
pub struct DelimiterRule;

impl<K, E> LexRule<K, E> for DelimiterRule
where
    K: TokenKind,
    E: LexError,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        match chars.peek() {
            Some(&(pos, ch)) => {
                if let Some(kind) = config.lookup_delimiter(ch) {
                    chars.next();
                    Some(Ok(Token::new(kind.clone(), Span::at(pos))))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        "delimiter"
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

    const RULE: DelimiterRule = DelimiterRule;

    #[test]
    fn delimiter_rule_matches_configured() {
        let config = cfg().with_delimiter('(', MockKind::Delim('('));
        let mut chars = "( rest".char_indices().peekable();
        let result = DelimiterRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Delim('('));
        assert_eq!(token.span, Span::at(0));
    }

    #[test]
    fn delimiter_rule_no_match_unconfigured() {
        let config = cfg();
        let mut chars = "( rest".char_indices().peekable();
        let result = DelimiterRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn delimiter_rule_no_match_letter() {
        let config = cfg().with_delimiter('(', MockKind::Delim('('));
        let mut chars = "hello".char_indices().peekable();
        let result = DelimiterRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn delimiter_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "delimiter");
    }

    #[test]
    fn delimiter_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "delimiter");
    }
}
