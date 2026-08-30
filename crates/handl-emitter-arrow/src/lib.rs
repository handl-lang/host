//! # handl-emitter-arrow
//!
//! Emitter for the **arrow function literal** construct — the **grammar half**
//! (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-arrow` (grammar) into a data-driven [`Language`]
//! fragment. The existing `ArrowRule` parses `(params) -> body` (and the
//! return-type-annotated variant `(params): RetType -> body`) as an infix
//! `->` operator, falling back to parenthesized grouping `(expr)` when the
//! content is not a parameter list.
//!
//! ## Grammar
//!
//! An infix rule for `token.arrow` (`->`) producing a `"function"` node
//! (arity 2: params + body). Binding power `(31, 32)` matches the existing
//! `ArrowRule` precedence.
//!
//! **Delimited/complex form:** `(params) -> body` requires parenthesized
//! param-list extraction and an infix arrow operator — the current Pratt
//! engine's grammar row format cannot handle this. A grammar row is emitted
//! as a **placeholder**. The full function-literal syntax requires a grammar
//! engine that supports param-list extraction. The **eval scheme** is
//! provided by the companion crate `handl-emitter-function-literal`.
//!
//! ## Eval (provided by `handl-emitter-function-literal`)
//!
//! This crate emits **only the token and grammar row** — it is the "grammar
//! half" of the function literal. The `"function"` node kind's eval scheme
//! (`Foreign{name:"make_closure", args:[Child(0), Child(1)]}`) is emitted by
//! `handl-emitter-function-literal`, the "eval half". This split lets the two
//! crates compose without fragment collisions when merged into a kit.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `ArrowRule` produces `AstNode{kind:"function",
//! payload:{"params":["x",...], "body":<ast>}}`. The data-driven path uses
//! the same node kind (`"function"`) and the `make_closure` host capability
//! creates an equivalent closure value. Parity is on the **evaluated value**
//! when the closure is applied.

#![warn(missing_docs)]

use handl_emitters::{declare_token, infix_binary, Emitter};
use handl_language::{LangError, Language};

/// Emitter for the arrow function literal construct (grammar half).
#[derive(Debug, Default, Clone, Copy)]
pub struct ArrowEmitter;

impl ArrowEmitter {
    /// Create a new arrow emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for ArrowEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the arrow function literal's grammar [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `->` (shared with the function emitter; re-declared here so this
    // crate is self-contained).
    declare_token(&mut lang, "token.arrow")?;

    // Grammar: infix rule for `->` (arity 2 — simplified; the Pratt engine
    // cannot handle param-list extraction). Binding power (31, 32) matches
    // the existing ArrowRule.
    infix_binary(&mut lang, "grammar.arrow.infix", "token.arrow", "function", 31, 32)?;

    // NOTE: the eval scheme for the `"function"` node kind is emitted by the
    // companion crate `handl-emitter-function-literal` to avoid collisions.
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
        assert_eq!(lang.eval.len(), 0);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = ArrowEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
