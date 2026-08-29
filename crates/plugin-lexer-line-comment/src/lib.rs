//! [`LineCommentRule`] — matches line comments.
//!
//! This crate owns the line comment lexing logic (`lex_line_comment`,
//! `starts_with`) and the [`CommentToken`] trait that comment-capable
//! token kind factories implement.

use handl_plugin_lexer::{LexError, LexRule, LexerConfig, Span, Token, TokenKind};
use std::iter::Peekable;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the iterator is positioned at the start of `prefix`.
pub fn starts_with(chars: &mut Peekable<std::str::CharIndices<'_>>, prefix: &str) -> bool {
    let mut peek = chars.clone();
    for expected in prefix.chars() {
        match peek.next() {
            Some((_, ch)) if ch == expected => {}
            _ => return false,
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Primitive
// ---------------------------------------------------------------------------

/// Lexes a line comment with the given `prefix`.
/// Returns the comment body (after `prefix`) and its span.
pub fn lex_line_comment(
    chars: &mut Peekable<std::str::CharIndices<'_>>,
    prefix: &str,
) -> (String, Span) {
    let (start, _) = chars.peek().copied().unwrap();
    let mut end = start;

    for _ in 0..prefix.chars().count() {
        let (pos, ch) = chars.next().unwrap();
        end = pos + ch.len_utf8();
    }

    let mut text = String::new();
    while let Some(&(pos, ch)) = chars.peek() {
        if ch == '\n' {
            break;
        }
        text.push(ch);
        end = pos + ch.len_utf8();
        chars.next();
    }

    (text, Span::new(start, end))
}

// ---------------------------------------------------------------------------
// Token trait
// ---------------------------------------------------------------------------

/// Token-kind factory capability for comments (line, block, and doc).
///
/// A host's token kind factory implements this so that comment rules
/// ([`LineCommentRule`], `BlockCommentRule`, `DocCommentRule`) can produce
/// comment tokens. The host decides which comment prefixes/delimiters to use
/// and how to turn comment text into a token kind.
pub trait CommentToken<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Builds a comment token kind from the comment text.
    fn comment_kind(&self, text: String) -> K;

    /// Builds a doc comment token kind from the doc comment text.
    fn doc_comment_kind(&self, text: String) -> K;

    /// Returns the line comment prefix string, if any (e.g. `"//"`).
    fn line_comment_prefix(&self) -> Option<&str> {
        None
    }

    /// Returns the block comment (start, end) strings, if any (e.g. `("/*", "*/")`).
    fn block_comment_delims(&self) -> Option<(&str, &str)> {
        None
    }

    /// Returns the doc line comment prefix string, if any (e.g. `"///"`).
    fn doc_line_comment_prefix(&self) -> Option<&str> {
        None
    }

    /// Returns the doc block comment (start, end) strings, if any (e.g. `("/**", "*/")`).
    fn doc_block_comment_delims(&self) -> Option<(&str, &str)> {
        None
    }
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// Matches line comments using [`CommentToken::line_comment_prefix`] and
/// [`lex_line_comment`].
#[derive(Clone, Default)]
pub struct LineCommentRule;

impl<K, E> LexRule<K, E> for LineCommentRule
where
    K: handl_plugin_lexer::CommentTokenKind,
    K::Factory: CommentToken<K>,
    E: LexError,
{
    fn try_lex(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        let factory = config.factory();
        let prefix = factory.line_comment_prefix()?;
        let doc_prefix = factory.doc_line_comment_prefix();

        // Don't match if this is actually a doc line comment.
        if let Some(doc) = doc_prefix {
            if starts_with(chars, doc) {
                return None;
            }
        }

        if !starts_with(chars, prefix) {
            return None;
        }

        let (text, span) = lex_line_comment(chars, prefix);
        Some(Ok(Token::new(factory.comment_kind(text), span)))
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
    use handl_plugin_lexer::{CommentTokenKind, DefaultLexError, LexRule, Span, TokenKind};
    use handl_plugin_lexer_common::{MockFactory, MockKind, empty_config as cfg};

    // Orphan-rule-allowed impl: CommentToken is local to this crate.
    impl CommentToken<MockKind> for MockFactory {
        fn comment_kind(&self, text: String) -> MockKind {
            (self.comment_kind_fn())(text)
        }

        fn doc_comment_kind(&self, text: String) -> MockKind {
            (self.comment_kind_fn())(text)
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
