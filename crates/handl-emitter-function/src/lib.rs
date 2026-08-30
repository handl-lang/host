//! # handl-emitter-function
//!
//! Emitter for the **function literal** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-function-literal` (grammar) and
//! `plugin-eval-function` (eval) into data-driven [`Language`] fragments.
//!
//! ## Eval
//!
//! The `"function"` node kind maps to `Prim{op:"closure", args:[]}`. The
//! tree-walk engine's `closure` primitive reads `params` (array of strings)
//! and `body` (arena index) from `node.payload`, captures the current scope,
//! and creates a closure value:
//! ```json
//! {"__closure__": true, "params": [...], "body": <idx>, "env": {...}}
//! ```
//! This closure is later applied by the `call` node's `App` scheme.
//!
//! ## Grammar simplification
//!
//! The existing `ArrowRule` parses `(params) -> body` as an infix rule for
//! `->`. The current Pratt engine's grammar row format cannot handle this
//! syntax (parenthesized param lists, arrow operator with param extraction).
//! A grammar row is emitted for `token.arrow` (infix, arity 2) as a
//! placeholder, but the full function-literal syntax requires a grammar
//! engine that supports param-list extraction. The **eval scheme** is the
//! fully migrated part; parity tests construct function ASTs manually with
//! the payload shape `{"params": ["x", ...], "body": <arena_idx>}`.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `FunctionRule` produces `AstNode{kind:"function",
//! payload:{"params":["x",...], "body":<ast>}}` and creates a function value
//! with `params`/`body`/`captured`. The data-driven path uses the same
//! payload fields (`params`/`body`) and the `closure` primitive creates an
//! equivalent closure value. Both evaluate to the same final `Value` when
//! applied.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, infix_binary, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the function literal construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct FunctionEmitter;

impl FunctionEmitter {
    /// Create a new function emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for FunctionEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the function literal construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();
    declare_token(&mut lang, "token.arrow")?;
    // Grammar: infix rule for `->` (arity 2 — simplified; the Pratt engine
    // cannot handle param-list extraction). Binding power (31, 32) matches
    // the existing ArrowRule.
    infix_binary(&mut lang, "grammar.arrow.infix", "token.arrow", "function", 31, 32)?;
    // Eval: closure primitive reads params/body from node.payload.
    eval_scheme(&mut lang, "eval.function", "function", Scheme::prim("closure", vec![]))?;
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
        let e = FunctionEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
