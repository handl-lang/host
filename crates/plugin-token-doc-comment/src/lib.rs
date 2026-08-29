#![allow(non_snake_case)]
use handl_plugin_lexer::{
    CommentTokenKind, DefaultLexError, LexError, LexRule, LexerConfig, Span, Token, TokenKind,
};
use handl_plugin_lexer_line_comment::{
    lex_line_comment, starts_with, CommentToken,
};
use std::fmt;
use std::iter::Peekable;

/// Token kind for canonical doc-comment tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum DocCommentKind {
    /// A doc comment token.
    DocComment(String),
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for DocCommentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocCommentKind::DocComment(v) => write!(f, "doc-comment({:?})", v),
            DocCommentKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for DocCommentKind {
    fn is_eof(&self) -> bool {
        matches!(self, DocCommentKind::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }

    type Factory = TokenFactory;
}

impl CommentTokenKind for DocCommentKind {
    fn is_comment(&self) -> bool {
        matches!(self, DocCommentKind::DocComment(_))
    }
}

pub fn DocComment_token(value: String, span: Span) -> Token<DocCommentKind> {
    Token::new(DocCommentKind::DocComment(value), span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_plugin_lexer::EofFactory<DocCommentKind> for TokenFactory {
    fn eof_kind(&self) -> DocCommentKind {
        DocCommentKind::Eof
    }
}

impl CommentToken<DocCommentKind> for TokenFactory {
    fn comment_kind(&self, text: String) -> DocCommentKind {
        DocCommentKind::DocComment(text)
    }

    fn doc_comment_kind(&self, text: String) -> DocCommentKind {
        DocCommentKind::DocComment(text)
    }

    fn doc_line_comment_prefix(&self) -> Option<&str> {
        Some("///")
    }

    fn doc_block_comment_delims(&self) -> Option<(&str, &str)> {
        Some(("/**", "*/"))
    }
}

// ---------------------------------------------------------------------------
// Doc comment primitives
// ---------------------------------------------------------------------------

/// Returns `true` if the iterator is at the start of a C-style `///` doc line comment.
pub fn is_doc_line_comment_start(chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool {
    starts_with(chars, "///")
}

/// Lexes a `///` doc line comment and returns its body and span.
pub fn lex_doc_line_comment(
    chars: &mut Peekable<std::str::CharIndices<'_>>,
) -> (String, Span) {
    lex_line_comment(chars, "///")
}

/// Returns `true` if the iterator is at the start of a C-style `/** */` doc block comment.
pub fn is_doc_block_comment_start(chars: &mut Peekable<std::str::CharIndices<'_>>) -> bool {
    starts_with(chars, "/**") && !starts_with(chars, "/**/")
}

/// Lexes a `/** */` doc block comment and returns its body and span.
pub fn lex_doc_block_comment(
    chars: &mut Peekable<std::str::CharIndices<'_>>,
) -> Result<(String, Span), DefaultLexError> {
    handl_plugin_lexer_block_comment::lex_block_comment(chars, "/**", "*/")
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// [`LexRule`] implementation that matches doc line and block comments.
#[derive(Clone, Default)]
pub struct DocCommentRule;

impl<K, E> LexRule<K, E> for DocCommentRule
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

        if let Some(doc_prefix) = factory.doc_line_comment_prefix() {
            if starts_with(chars, doc_prefix) {
                let (text, span) = lex_line_comment(chars, doc_prefix);
                return Some(Ok(Token::new(factory.doc_comment_kind(text), span)));
            }
        }

        if let Some((doc_open, doc_close)) = factory.doc_block_comment_delims() {
            if starts_with(chars, doc_open) {
                return Some(
                    handl_plugin_lexer_block_comment::lex_block_comment(chars, doc_open, doc_close)
                        .map(|(text, span)| {
                            Token::new(factory.doc_comment_kind(text), span)
                        })
                        .map_err(E::from),
                );
            }
        }

        None
    }

    fn name(&self) -> &'static str {
        "doc_comment"
    }

    fn clone_box(&self) -> Box<dyn LexRule<K, E>> {
        Box::new(self.clone())
    }
}
