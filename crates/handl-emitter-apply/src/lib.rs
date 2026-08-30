//! # handl-emitter-apply
//!
//! Emitter for the **parenthesized grouping / call disambiguation** construct
//! (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-apply` (grammar) into a data-driven [`Language`]
//! fragment. The existing `ApplyRule` parses `(expr)` as a grouping that
//! returns the inner expression unchanged.
//!
//! ## Grammar
//!
//! A prefix rule for `token.lparen` (`(`) producing an `"apply"` node
//! (arity 1). The prefix right binding power is 0, so the inner expression
//! parses at the lowest precedence.
//!
//! **Delimited form:** `(expr)` requires a closing `)` after the inner
//! expression. The current Pratt engine's grammar row format cannot handle
//! closing delimiters — a grammar row is emitted as a **placeholder**. The
//! full `(expr)` syntax requires a grammar engine that supports delimited
//! forms. The **eval scheme** is the fully migrated part; parity tests
//! construct `apply` ASTs manually.
//!
//! ## Eval
//!
//! The `"apply"` node kind maps to `Child(0)` — an apply/grouping node just
//! evaluates to its inner child. Grouping only affects parse-time precedence,
//! not runtime semantics.
//!
//! ## Overlap with `handl-emitter-paren`
//!
//! This construct overlaps with `handl-emitter-paren` (both handle `(expr)`).
//! The `apply` crate is the "call/grouping disambiguation" rule from
//! `plugin-grammar-apply`; `paren` is the pure grouping case from
//! `plugin-grammar-primary-paren`. In the data-driven model they collapse to
//! the same semantics — both evaluate to the inner expression. They are kept
//! as separate crates for source-migration traceability; a kit should include
//! only one of the two to avoid fragment collisions (they declare the same
//! `token.lparen` and would collide on the grammar row id if both used the
//! same id). This crate uses distinct fragment ids (`grammar.apply.prefix`,
//! `eval.apply`) so it composes with `paren` only if the kit deduplicates the
//! shared `token.lparen`.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `ApplyRule` returns the inner `AstNode` unchanged (no wrapping
//! node). The data-driven path produces an `"apply"` node that evaluates to
//! `Child(0)`. Parity is on the **evaluated value** (e.g. `(1 + 2) = 3`).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the parenthesized grouping / call disambiguation construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct ApplyEmitter;

impl ApplyEmitter {
    /// Create a new apply emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for ApplyEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the apply/grouping construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `(` (shared with the paren emitter; re-declared here so this
    // crate is self-contained).
    declare_token(&mut lang, "token.lparen")?;

    // Grammar: prefix rule for `(` (arity 1, prefix_rbp 0). This is a
    // placeholder — the Pratt engine cannot handle the closing `)` delimiter.
    prefix_compound(&mut lang, "grammar.apply.prefix", "token.lparen", "apply", 1, 0)?;

    // Eval: Child(0) — an apply node evaluates to its inner child.
    eval_scheme(&mut lang, "eval.apply", "apply", Scheme::child(0))?;

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
        let e = ApplyEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
