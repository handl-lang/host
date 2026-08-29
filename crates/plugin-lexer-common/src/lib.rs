//! # handl-plugin-lexer-common
//!
//! Shared test infrastructure and token mock types for the lexer crates.
//!
//! Provides:
//! - **Token mocks**: [`MockKind`], [`MockFactory`] (configurable), [`MockLexer`].
//! - **Rule mocks**: [`RuleMock`], [`BangRule`], [`HashRule`], [`AtRule`].
//! - **Config builders**: [`empty_config`], [`test_config`].
//!
//! Depends on `handl-plugin-lexer` for core types (`LexRule`, `LexerConfig`, `Token`, etc.).

mod config;
mod rule_mocks;
mod tokens;

pub use config::{empty_config, test_config};
pub use rule_mocks::{AtRule, BangRule, HashRule, RuleMock};
pub use tokens::{MockFactory, MockKind, MockLexer};
