//! # handl-emitter-paren
//!
//! Emitter for the **parenthesized grouping** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-paren` (grammar stub) into a data-driven
//! [`Language`] fragment. The source crate is a 20-line stub — the grammar
//! rule was never implemented in Rust. This emitter creates the data-driven
//! version from scratch based on the language design: `(expr)` is a grouping
//! prefix operator.
//!
//! ## Grammar
//!
//! A prefix rule for `token.lparen` (`(`) producing a `"paren"` node
//! (arity 1). The prefix right binding power is 0, so the inner expression
//! parses at the lowest precedence (any operator can appear inside).
//!
//! **Delimited form:** `(expr)` requires a closing `)` after the inner
//! expression. The current Pratt engine's grammar row format cannot handle
//! closing delimiters — a grammar row is emitted as a **placeholder**. The
//! full `(expr)` syntax requires a grammar engine that supports delimited
//! forms. The **eval scheme** is the fully migrated part; parity tests
//! construct `paren` ASTs manually.
//!
//! ## Eval
//!
//! The `"paren"` node kind maps to `Child(0)` — a paren node just evaluates
//! to its inner child. Grouping only affects parse-time precedence, not
//! runtime semantics.
//!
//! ## Parity
//!
//! The existing `plugin-grammar-primary-paren` is a stub with no Rust rule.
//! The data-driven path is the first implementation. Parity is on the
//! **evaluated value** (e.g. `(1 + 2) = 3`).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the parenthesized grouping construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct ParenEmitter;

impl ParenEmitter {
    /// Create a new paren emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for ParenEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the parenthesized grouping construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `(` (already declared in the tokens emitter; re-declared here
    // so this crate is self-contained).
    declare_token(&mut lang, "token.lparen")?;

    // Grammar: prefix rule for `(` (arity 1, prefix_rbp 0). This is a
    // placeholder — the Pratt engine cannot handle the closing `)` delimiter.
    // Full `(expr)` syntax requires a delimited grammar engine.
    prefix_compound(&mut lang, "grammar.paren.prefix", "token.lparen", "paren", 1, 0)?;

    // Eval: Child(0) — a paren node evaluates to its inner child.
    eval_scheme(&mut lang, "eval.paren", "paren", Scheme::child(0))?;

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
        let e = ParenEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
