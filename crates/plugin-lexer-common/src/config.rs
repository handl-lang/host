//! Config builders for tests.

use crate::tokens::{MockFactory, MockKind};
use handl_plugin_lexer::{DefaultLexError, LexerConfig};

/// Returns an empty `LexerConfig<MockKind, DefaultLexError>` with just the
/// factory — no rules, no keywords, no delimiters. Use this when tests need
/// full control.
pub fn empty_config() -> LexerConfig<MockKind, DefaultLexError> {
    LexerConfig::new(MockFactory::new())
}

/// Returns a `LexerConfig<MockKind, DefaultLexError>` with a few common
/// keywords and delimiters pre-configured. Rules must be added by the caller
/// from the individual `plugin-lexer-*` rule crates.
pub fn test_config() -> LexerConfig<MockKind, DefaultLexError> {
    LexerConfig::new(MockFactory::new())
        .with_keyword("true", MockKind::Bool(true))
        .with_keyword("false", MockKind::Bool(false))
        .with_keyword("null", MockKind::Null)
        .with_delimiter('(', MockKind::Delim('('))
        .with_delimiter(')', MockKind::Delim(')'))
        .with_delimiter('{', MockKind::Delim('{'))
        .with_delimiter('}', MockKind::Delim('}'))
        .with_delimiter('[', MockKind::Delim('['))
        .with_delimiter(']', MockKind::Delim(']'))
        .with_delimiter(',', MockKind::Comma)
        .with_delimiter(':', MockKind::Colon)
}
