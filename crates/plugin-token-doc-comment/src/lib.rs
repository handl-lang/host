#![allow(non_snake_case)]
use handl_lexer::primitives;
use handl_lexer::{
    CommentFactory, CommentTokenKind, DefaultLexError, LexError, LexerConfig, LexRule, Span, Token,
    TokenKind,
};
use std::fmt;

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

pub fn DocComment_token(value: String, span: Span) -> Token<DocCommentKind> {
    Token::new(DocCommentKind::DocComment(value), span)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenFactory;

impl handl_lexer::EofFactory<DocCommentKind> for TokenFactory {
    fn eof_kind(&self) -> DocCommentKind {
        DocCommentKind::Eof
    }
}

/// [`LexRule`] implementation that matches doc line and block comments.
#[derive(Clone, Default)]
pub struct DocCommentRule;

impl<K, E> LexRule<K, E> for DocCommentRule
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
        if config.factory().is_doc_line_comment_start(chars) {
            let (text, span) = config.factory().lex_doc_line_comment(chars);
            return Some(Ok(Token::new(config.factory().doc_comment_kind(text), span)));
        }

        if config.factory().is_doc_block_comment_start(chars) {
            return Some(
                config
                    .factory()
                    .lex_doc_block_comment(chars)
                    .map(|(text, span)| Token::new(config.factory().doc_comment_kind(text), span))
                    .map_err(E::from),
            );
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

/// Returns `true` if the iterator is at the start of a C-style `///` doc line comment.
pub fn is_doc_line_comment_start(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> bool {
    primitives::starts_with(chars, "///")
}

/// Lexes a `///` doc line comment and returns its body and span.
pub fn lex_doc_line_comment(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> (String, Span) {
    primitives::lex_line_comment(chars, "///")
}

/// Returns `true` if the iterator is at the start of a C-style `/** */` doc block comment.
pub fn is_doc_block_comment_start(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> bool {
    primitives::starts_with(chars, "/**") && !primitives::starts_with(chars, "/**/")
}

/// Lexes a `/** */` doc block comment and returns its body and span.
pub fn lex_doc_block_comment(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<(String, Span), DefaultLexError> {
    primitives::lex_block_comment(chars, "/**", "*/")
}
