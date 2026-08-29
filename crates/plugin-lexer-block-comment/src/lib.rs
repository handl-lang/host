//! [`BlockCommentRule`] — matches block comments.

use handl_plugin_lexer::{
    CommentFactory, CommentTokenKind, DefaultLexError, LexError, LexRule, LexerConfig, Token,
};

/// Matches block comments using the factory's [`CommentFactory::is_block_comment_start`]
/// and [`CommentFactory::lex_block_comment`].
#[derive(Clone, Default)]
pub struct BlockCommentRule;

impl<K, E> LexRule<K, E> for BlockCommentRule
where
    K: CommentTokenKind,
    K::Factory: CommentFactory<K>,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        if !config.factory().is_block_comment_start(chars) {
            return None;
        }

        Some(
            config
                .factory()
                .lex_block_comment(chars)
                .map(|(text, span)| Token::new(config.factory().comment_kind(text), span))
                .map_err(E::from),
        )
    }

    fn name(&self) -> &'static str {
        "block_comment"
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

    const RULE: BlockCommentRule = BlockCommentRule;

    #[test]
    fn block_comment_rule_matches() {
        let config = cfg();
        let mut chars = "/* hello */ rest".char_indices().peekable();
        let result = BlockCommentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Comment(" hello ".into()));
        assert_eq!(token.span, Span::new(0, 11));
    }

    #[test]
    fn block_comment_rule_nested_stars() {
        let config = cfg();
        let mut chars = "/* a * b */ rest".char_indices().peekable();
        let result = BlockCommentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Comment(" a * b ".into()));
    }

    #[test]
    fn block_comment_rule_unterminated_errors() {
        let config = cfg();
        let mut chars = "/* oops".char_indices().peekable();
        let result = BlockCommentRule.try_lex(&mut chars, &config);
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn block_comment_rule_no_match_single_slash() {
        let config = cfg();
        let mut chars = "/ hello".char_indices().peekable();
        let result = BlockCommentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn block_comment_rule_no_match_line_comment() {
        let config = cfg();
        let mut chars = "// hello".char_indices().peekable();
        let result = BlockCommentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn block_comment_rule_no_match_letter() {
        let config = cfg();
        let mut chars = "hello".char_indices().peekable();
        let result = BlockCommentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn block_comment_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "block_comment");
    }

    #[test]
    fn block_comment_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "block_comment");
    }
}
