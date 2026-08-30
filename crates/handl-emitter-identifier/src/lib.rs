//! # handl-emitter-identifier
//!
//! Emitter for the **identifier** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-identifier` (grammar) and the `var` rule
//! from `plugin-eval-core` (eval) into data-driven [`Language`] fragments.
//!
//! ## Grammar
//!
//! A prefix rule for `token.identifier` tokens producing a `"var"` node
//! (arity 0). The Pratt engine sets the atom payload to the token text (a
//! string), which the eval scheme reads via `Var("$payload")`.
//!
//! ## Eval
//!
//! The `"var"` node kind maps to `Var("$payload")`: the tree-walk engine
//! reads the variable name from `node.payload` (the token text) and looks it
//! up in the scope stack.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `IdentifierRule` produces `AstNode{kind:"var",
//! payload:{"name": <text>}}`. The data-driven Pratt engine produces
//! `AstNode{kind:"var", payload:<text>}` (atom convention). The eval scheme
//! `Var("$payload")` reads the name from the payload directly. Both paths
//! evaluate to the same final `Value` — the parity test compares evaluated
//! values, not AST payload shapes.

#![warn(missing_docs)]

use handl_emitters::{
    declare_token, eval_scheme, prefix_atom, Emitter,
};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the identifier construct.
///
/// Produces grammar + eval fragments encoding identifier reference semantics.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdentifierEmitter;

impl IdentifierEmitter {
    /// Create a new identifier emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for IdentifierEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the identifier construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();
    declare_token(&mut lang, "token.identifier")?;
    prefix_atom(&mut lang, "grammar.identifier.prefix", "token.identifier", "var")?;
    eval_scheme(&mut lang, "eval.var", "var", Scheme::var("$payload"))?;
    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 1);
        assert_eq!(lang.grammar.len(), 1);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = IdentifierEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
