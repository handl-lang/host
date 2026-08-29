//! [`BlockCommentRule`] — matches block comments.
//!
//! This crate owns the block comment lexing logic (`lex_block_comment`).
//! The [`CommentToken`] trait is shared from `plugin-lexer-line-comment`.

use handl_plugin_lexer::{
    CommentTokenKind, DefaultLexError, LexError, LexRule, LexerConfig, Span, Token,
};
use handl_plugin_lexer_line_comment::{starts_with, CommentToken};
use std::iter::Peekable;

// ---------------------------------------------------------------------------
// Primitive
// ---------------------------------------------------------------------------

/// Lexes a block comment with the given `open` and `close` delimiters.
/// Returns the comment text and its span, or an error if unterminated.
pub fn lex_block_comment(
    chars: &mut Peekable<std::str::CharIndices<'_>>,
    open: &str,
    close: &str,
) -> Result<(String, Span), DefaultLexError> {
    let (start, _) = chars.peek().copied().unwrap();
    let mut end = start;

    for _expected in open.chars() {
        let (pos, ch) = chars.next().unwrap();
        let _ = ch; // assume caller has verified the prefix
        end = pos + ch.len_utf8();
    }

    let mut close_chars = close.chars();
    let close_first = close_chars.next().unwrap();
    let close_second = close_chars.next().unwrap();

    let mut text = String::new();
    loop {
        match chars.next() {
            None => {
                return Err(DefaultLexError::UnterminatedString {
                    span: Span::new(start, end),
                });
            }
            Some((pos, ch)) if ch == close_first => {
                end = pos + ch.len_utf8();
                match chars.peek() {
                    Some(&(_, c)) if c == close_second => {
                        let (_, c) = chars.next().unwrap();
                        end += c.len_utf8();
                        break;
                    }
                    _ => text.push(ch),
                }
            }
            Some((pos, ch)) => {
                text.push(ch);
                end = pos + ch.len_utf8();
            }
        }
    }

    Ok((text, Span::new(start, end)))
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// Matches block comments using [`CommentToken::block_comment_delims`] and
/// [`lex_block_comment`].
#[derive(Clone, Default)]
pub struct BlockCommentRule;

impl<K, E> LexRule<K, E> for BlockCommentRule
where
    K: CommentTokenKind,
    K::Factory: CommentToken<K>,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        let factory = config.factory();
        let (open, close) = factory.block_comment_delims()?;

        // Don't match if this is actually a doc block comment.
        if let Some((doc_open, _)) = factory.doc_block_comment_delims() {
            if starts_with(chars, doc_open) {
                return None;
            }
        }

        if !starts_with(chars, open) {
            return None;
        }

        Some(
            lex_block_comment(chars, open, close)
                .map(|(text, span)| Token::new(factory.comment_kind(text), span))
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
    use handl_plugin_lexer::{
        CommentTokenKind, DefaultLexError, EofFactory, LexRule, LexerConfig, Span, Token,
        TokenKind,
    };
    use handl_plugin_lexer_line_comment::CommentToken;
    use std::fmt;

    // Local mock for block-comment tests (CommentToken is not local to this
    // crate, so we cannot impl it for the shared MockFactory via orphan rule).
    #[derive(Debug, Clone, PartialEq)]
    enum MockKind {
        Comment(String),
        Eof,
    }

    impl fmt::Display for MockKind {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                MockKind::Comment(s) => write!(f, "comment({s:?})"),
                MockKind::Eof => write!(f, "eof"),
            }
        }
    }

    impl TokenKind for MockKind {
        type Factory = MockFactory;
        fn is_eof(&self) -> bool {
            matches!(self, MockKind::Eof)
        }
        fn is_error(&self) -> bool {
            false
        }
    }

    impl CommentTokenKind for MockKind {
        fn is_comment(&self) -> bool {
            matches!(self, MockKind::Comment(_))
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    struct MockFactory;

    impl EofFactory<MockKind> for MockFactory {
        fn eof_kind(&self) -> MockKind {
            MockKind::Eof
        }
    }

    impl CommentToken<MockKind> for MockFactory {
        fn comment_kind(&self, text: String) -> MockKind {
            MockKind::Comment(text)
        }

        fn doc_comment_kind(&self, text: String) -> MockKind {
            MockKind::Comment(text)
        }

        fn line_comment_prefix(&self) -> Option<&str> {
            Some("//")
        }

        fn block_comment_delims(&self) -> Option<(&str, &str)> {
            Some(("/*", "*/"))
        }

        fn doc_line_comment_prefix(&self) -> Option<&str> {
            Some("///")
        }

        fn doc_block_comment_delims(&self) -> Option<(&str, &str)> {
            Some(("/**", "*/"))
        }
    }

    fn cfg() -> LexerConfig<MockKind, DefaultLexError> {
        LexerConfig::new(MockFactory)
    }

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
