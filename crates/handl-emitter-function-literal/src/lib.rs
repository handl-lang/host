//! # handl-emitter-function-literal
//!
//! Emitter for the **function literal** construct — the **eval half**
//! (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-function-literal` (eval semantics) into a
//! data-driven [`Language`] fragment. The `plugin-grammar-function-literal`
//! crate is essentially the same construct as `plugin-grammar-arrow` (both
//! handle `->`). Rather than duplicate the token and grammar rows, this
//! crate emits **only the eval scheme** for the `"function"` node kind — it
//! is the "eval half" of the function literal, while `handl-emitter-arrow`
//! is the "grammar half" (token + grammar row). This split lets the two
//! crates compose without fragment collisions when merged into a kit.
//!
//! ## Eval
//!
//! The `"function"` node kind maps to
//! `Foreign{name:"make_closure", args:[Child(0), Child(1)]}`. Closure
//! creation is a host capability: the `make_closure` host function reads the
//! parameter list (child 0) and body (child 1), captures the current
//! environment, and produces a closure value. This is a simplified scheme —
//! the real function eval creates a closure capturing the environment; the
//! `Foreign` variant models closure creation as a host capability.
//!
//! ## Parity
//!
//! The existing `FunctionRule` produces `AstNode{kind:"function",
//! payload:{"params":["x",...], "body":<ast>}}` and creates a function value
//! with `params`/`body`/`captured`. The data-driven path uses the same node
//! kind and the `make_closure` host capability creates an equivalent closure
//! value. Parity is on the **evaluated value** when the closure is applied.

#![warn(missing_docs)]

use handl_emitters::{eval_scheme, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the function literal construct (eval half).
#[derive(Debug, Default, Clone, Copy)]
pub struct FunctionLiteralEmitter;

impl FunctionLiteralEmitter {
    /// Create a new function-literal emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for FunctionLiteralEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the function literal's eval [`Language`] fragment.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Eval: Foreign{make_closure, [Child(0), Child(1)]} — closure creation is
    // a host capability. Child(0) is the parameter list, Child(1) is the body.
    // No tokens or grammar rows are emitted here — those come from the
    // companion crate `handl-emitter-arrow`.
    eval_scheme(
        &mut lang,
        "eval.function",
        "function",
        Scheme::foreign("make_closure", vec![Scheme::child(0), Scheme::child(1)]),
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 0);
        assert_eq!(lang.grammar.len(), 0);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = FunctionLiteralEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
