//! # handl-emitter-value
//!
//! Emitter for the **literal value** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-value` (grammar) and `plugin-eval-value`
//! (eval) into data-driven [`Language`] fragments.
//!
//! ## Grammar
//!
//! A prefix rule for `token.number` tokens producing a `"value"` node
//! (arity 0). The Pratt engine interprets the token text as a number (since
//! the token kind id contains `"number"`) and sets that as the atom payload.
//!
//! ## Eval
//!
//! The `"value"` node kind has **no eval scheme** — the tree-walk engine's
//! no-scheme fallback returns `node.payload` directly. This matches the
//! existing `ValueRule` which extracts `payload["value"]` and returns it.
//! In the data-driven path, the payload *is* the value (atom convention),
//! so the fallback returns the correct value directly.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `ValueRule` produces `AstNode{kind:"value",
//! payload:{"value": <v>}}` and eval extracts `payload["value"]`. The
//! data-driven Pratt engine produces `AstNode{kind:"value", payload:<v>}`
//! (atom convention), and the tree-walk no-scheme fallback returns
//! `payload` directly. Both paths evaluate to the same final `Value`.

#![warn(missing_docs)]

use handl_emitters::{declare_token, prefix_atom, Emitter};
use handl_language::{LangError, Language};

/// Emitter for the literal value construct.
///
/// Produces grammar fragments for value literals. No eval scheme is needed
/// (the tree-walk no-scheme fallback returns the payload directly).
#[derive(Debug, Default, Clone, Copy)]
pub struct ValueEmitter;

impl ValueEmitter {
    /// Create a new value emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for ValueEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the value construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();
    declare_token(&mut lang, "token.number")?;
    prefix_atom(&mut lang, "grammar.value.number", "token.number", "value")?;

    // String literals: prefix atom producing a "value" node.
    // The Pratt engine's atom_payload returns the token text as a string
    // for non-number token kinds, which is correct for string literals
    // (the lexer strips the quotes).
    declare_token(&mut lang, "token.string")?;
    prefix_atom(&mut lang, "grammar.value.string", "token.string", "value")?;

    // Bool literals: prefix atom producing a "value" node.
    // The Pratt engine's atom_payload interprets "true"/"false" as
    // boolean values when the token kind id contains "bool".
    declare_token(&mut lang, "token.bool")?;
    prefix_atom(&mut lang, "grammar.value.bool", "token.bool", "value")?;

    // Null literal: prefix atom producing a "value" node.
    // The Pratt engine's atom_payload returns Value::Null when the
    // token kind id contains "null".
    declare_token(&mut lang, "token.null")?;
    prefix_atom(&mut lang, "grammar.value.null", "token.null", "value")?;

    // No eval scheme: the tree-walk no-scheme fallback returns node.payload
    // directly, which is the interpreted value for atoms.
    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 4);
        assert_eq!(lang.grammar.len(), 4);
        // No eval scheme — relies on no-scheme fallback.
        assert_eq!(lang.eval.len(), 0);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = ValueEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
