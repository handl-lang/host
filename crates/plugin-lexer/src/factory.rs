//! Token factories for dialect-specific token construction.
//!
//! The framework only owns [`EofFactory`] — every lexer needs an EOF token.
//! All other token-category traits (number, string, ident, operator, comment,
//! keyword) live in their respective `plugin-lexer-*` crates.

use crate::TokenKind;

/// Factory for producing the end-of-input token.
///
/// This is the only factory trait kept in the framework crate because every
/// lexer needs an EOF token regardless of which token-category plugins it
/// composes.
pub trait EofFactory<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns the EOF token kind for the dialect.
    fn eof_kind(&self) -> K;
}
