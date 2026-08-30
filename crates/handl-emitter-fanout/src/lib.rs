//! # handl-emitter-fanout
//!
//! Emitter for the **arrow fanout operator** `&&&` (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-infix-fanout` (grammar) into a data-driven
//! [`Language`] fragment. The existing Rust rule desugars `f &&& g` into a
//! `call` node with a `fn.fanout_fn` callee at parse time at binding power
//! `(25, 25)` — right-associative. In the data-driven path we use a dedicated
//! `fanout` node kind with a `Foreign` eval scheme, rather than desugaring to
//! `call` at parse time. This is cleaner for the data-driven model: the
//! surface construct is preserved in the AST and the eval scheme expresses
//! the foreign call directly.
//!
//! ## Grammar
//!
//! Infix rule for `&&&` at binding power `(25, 25)` — **right-associative**
//! (`rbp == lbp`) so `f &&& g &&& h` parses as `f &&& (g &&& h)` — matching
//! the existing `FanoutRule` precedence (tighter than `>>>` at 20, looser
//! than pipe `|>` at 30 and comparison at 50). Produces a `fanout` node
//! (arity 2): child 0 is the left arrow, child 1 is the right arrow.
//!
//! ## Eval
//!
//! - `fanout` → `Foreign{name:"fn.fanout_fn", args:[Child(0), Child(1)]}`
//!
//! Desugars `f &&& g` into `fn.fanout_fn(f, g)` — a foreign (host) call that
//! returns a new arrow `x => [f(x), g(x)]` rather than an eager result. This
//! matches Haskell's `Control.Arrow` `&&&`.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `FanoutRule` desugars `f &&& g` to `call(fn.fanout_fn(f, g))`
//! at parse time; eval calls the `fn.fanout_fn` host function. The
//! data-driven path produces a `fanout` node with a
//! `Foreign{name:"fn.fanout_fn"}` scheme that evaluates to the same host
//! call. Parity is on the **evaluated value** (the fanout arrow), not the
//! AST node kind — the data path uses a dedicated `fanout` node kind, which
//! is expected and cleaner for the data-driven model.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, infix_binary, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the arrow fanout operator construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct FanoutEmitter;

impl FanoutEmitter {
    /// Create a new fanout emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for FanoutEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the arrow fanout operator's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kind for the fanout operator.
    declare_token(&mut lang, "token.fanout")?;

    // Grammar: infix rule, right-associative at BP (25, 25).
    infix_binary(&mut lang, "grammar.fanout.infix", "token.fanout", "fanout", 25, 25)?;

    // Eval: fanout node desugars to Foreign{name:"fn.fanout_fn", args:[Child(0), Child(1)]}.
    // `f &&& g` evaluates to `fn.fanout_fn(f, g)`.
    eval_scheme(
        &mut lang,
        "eval.fanout",
        "fanout",
        Scheme::foreign(
            "fn.fanout_fn",
            vec![Scheme::child(0), Scheme::child(1)],
        ),
    )?;

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
        let e = FanoutEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
