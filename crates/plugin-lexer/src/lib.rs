//! # handl-plugin-lexer
//!
//! Lexing abstraction for handl expressions.
//!
//! Defines the [`Lexer`] trait, [`Token`], [`TokenKind`], [`Span`], and
//! [`LexError`] — the types needed to turn raw expression text into a
//! token stream. The canonical implementation lives in `handl-lexer-canonical`.
//!
//! ## Architectural rule
//!
//! > **The lexer produces tokens; the parser consumes them.**
//!
//! The lexer is deliberately separate from the parser so that tooling
//! (LSP, formatters, linters) can depend on tokens without pulling in
//! the full parser and AST.

mod compose;
#[cfg(test)]
mod compose_test;
mod config;
#[cfg(test)]
mod config_test;
mod error;
mod factory;
#[cfg(test)]
mod factory_tests;
mod grammar_info;
#[cfg(test)]
mod grammar_info_tests;
mod lexer;
#[cfg(test)]
mod lexer_test;
pub mod primitives;
mod rule;
mod span;
mod token;
#[cfg(test)]
mod token_test;

pub use config::LexerConfig;
pub use error::{DefaultLexError, LexError};
pub use factory::*;
pub use grammar_info::{GrammarInfo, GrammarInfoProvider, KitGrammarMetadata};
pub use lexer::Lexer;
pub use rule::{FnRule, LexRule};
pub use span::Span;
pub use token::{DynToken, DynTokenKind, CommentTokenKind, Token, TokenKind};
