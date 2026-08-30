//! # handl-emitter-flow
//!
//! Emitter for the **arrow composition operator** `>>>` (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-infix-flow` (grammar) into a data-driven
//! [`Language`] fragment. The existing Rust rule desugars `f >>> g` into a
//! `call` node with a `fn.flow` callee at parse time at binding power
//! `(20, 20)` — right-associative. In the data-driven path we use a dedicated
//! `flow` node kind with a `Foreign` eval scheme, rather than desugaring to
//! `call` at parse time. This is cleaner for the data-driven model: the
//! surface construct is preserved in the AST and the eval scheme expresses
//! the foreign call directly.
//!
//! ## Grammar
//!
//! Infix rule for `>>>` at binding power `(20, 20)` — **right-associative**
//! (`rbp == lbp`) so `f >>> g >>> h` parses as `f >>> (g >>> h)` — matching
//! the existing `FlowRule` precedence (looser than arrow fanout `&&&` at 25
//! and pipe `|>` at 30, tighter than assignment `=` at 10). Produces a `flow`
//! node (arity 2): child 0 is the left arrow, child 1 is the right arrow.
//!
//! ## Eval
//!
//! - `flow` → `Foreign{name:"fn.flow", args:[Child(0), Child(1)]}`
//!
//! Desugars `f >>> g` into `fn.flow(f, g)` — a foreign (host) call that
//! returns a composed function (a new arrow) rather than an eager result.
//! This matches Haskell's `Control.Arrow` `>>>`.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `FlowRule` desugars `f >>> g` to `call(fn.flow(f, g))` at
//! parse time; eval calls the `fn.flow` host function. The data-driven path
//! produces a `flow` node with a `Foreign{name:"fn.flow"}` scheme that
//! evaluates to the same host call. Parity is on the **evaluated value**
//! (the composed arrow), not the AST node kind — the data path uses a
//! dedicated `flow` node kind, which is expected and cleaner for the
//! data-driven model.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, infix_binary, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the arrow composition operator construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct FlowEmitter;

impl FlowEmitter {
    /// Create a new flow emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for FlowEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the arrow composition operator's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kind for the flow operator.
    declare_token(&mut lang, "token.flow")?;

    // Grammar: infix rule, right-associative at BP (20, 20).
    infix_binary(&mut lang, "grammar.flow.infix", "token.flow", "flow", 20, 20)?;

    // Eval: flow node desugars to Foreign{name:"fn.flow", args:[Child(0), Child(1)]}.
    // `f >>> g` evaluates to `fn.flow(f, g)`.
    eval_scheme(
        &mut lang,
        "eval.flow",
        "flow",
        Scheme::foreign(
            "fn.flow",
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
        let e = FlowEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
