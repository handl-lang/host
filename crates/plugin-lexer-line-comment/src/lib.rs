//! [`LineCommentRule`] — matches line comments.

use handl_plugin_lexer::{CommentFactory, CommentTokenKind, LexError, LexerConfig, LexRule, Token};

/// Matches line comments using the factory's [`CommentFactory::is_line_comment_start`]
/// and [`CommentFactory::lex_line_comment`].
#[derive(Clone, Default)]
pub struct LineCommentRule;

impl<K, E> LexRule<K, E> for LineCommentRule
where
    K: CommentTokenKind,
    K::Factory: CommentFactory<K>,
    E: LexError,
{
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        if !config.factory().is_line_comment_start(chars) {
            return None;
        }

        let (text, span) = config.factory().lex_line_comment(chars);
        Some(Ok(Token::new(config.factory().comment_kind(text), span)))
    }

    fn name(&self) -> &'static str {
        "line_comment"
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

    const RULE: LineCommentRule = LineCommentRule;

    #[test]
    fn line_comment_rule_matches() {
        let config = cfg();
        let mut chars = "// hello\nrest".char_indices().peekable();
        let result = LineCommentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Comment(" hello".into()));
        assert_eq!(token.span, Span::new(0, 8));
    }

    #[test]
    fn line_comment_rule_matches_no_newline() {
        let config = cfg();
        let mut chars = "// hello".char_indices().peekable();
        let result = LineCommentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Comment(" hello".into()));
    }

    #[test]
    fn line_comment_rule_no_match_single_slash() {
        let config = cfg();
        let mut chars = "/ hello".char_indices().peekable();
        let result = LineCommentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn line_comment_rule_no_match_block_comment() {
        let config = cfg();
        let mut chars = "/* hello */".char_indices().peekable();
        let result = LineCommentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn line_comment_rule_no_match_letter() {
        let config = cfg();
        let mut chars = "hello".char_indices().peekable();
        let result = LineCommentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn line_comment_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "line_comment");
    }

    #[test]
    fn line_comment_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "line_comment");
    }
}
