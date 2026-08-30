//! # handl-emitter-handle
//!
//! Emitter for the **handle expression** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-handle` (grammar stub) and
//! `plugin-grammar-handle` (75-line real impl) + `plugin-eval-handle` (eval)
//! into data-driven [`Language`] fragments.
//!
//! ## Grammar
//!
//! A prefix rule for `token.keyword.handle` producing a `"handle"` node
//! (arity 2, placeholder). The real `HandleRule` parses `handle expr` — a
//! prefix operator that wraps an expression with effect handler clauses.
//! The full handle syntax includes a body and handler clauses (delimited
//! form). The current Pratt engine's grammar row format only supports
//! fixed-arity prefix operators; it cannot handle the clause delimiters. A
//! grammar row is emitted as a **placeholder** (arity 2 = body + clauses).
//! The full `handle` syntax requires a grammar engine that supports
//! delimited forms. The **eval scheme** is the fully migrated part; parity
//! tests construct `handle` ASTs manually.
//!
//! ## Eval
//!
//! The `"handle"` node kind maps to
//! `Foreign{name:"handle", args:[Child(0), Child(1)]}`. Handle is a complex
//! construct (effect handlers with resumable continuations), so it uses a
//! foreign (host) call. `Child(0)` is the body and `Child(1)` is the
//! handler clauses collection. Full effect-handler eval is a foreign/host
//! capability — the host runtime implements the `handle` foreign function,
//! which evaluates the body with the handler clauses installed, intercepting
//! effect operations.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `HandleRule` produces `AstNode{kind:"handle",
//! payload:{"expr": <ast>}}`. The data-driven path uses
//! `{"children": [body_idx, clauses_idx]}`. Both evaluate to the same final
//! `Value`. Parity is on the **evaluated value**.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the handle expression construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct HandleEmitter;

impl HandleEmitter {
    /// Create a new handle emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for HandleEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the handle expression construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `handle` keyword (already declared in the tokens emitter;
    // re-declared here so this crate is self-contained).
    declare_token(&mut lang, "token.keyword.handle")?;

    // Grammar: prefix rule for `handle` (arity 2 = body + clauses,
    // placeholder). Delimited form. The Pratt engine cannot handle the
    // clause delimiters.
    prefix_compound(
        &mut lang,
        "grammar.handle.prefix",
        "token.keyword.handle",
        "handle",
        2,
        0,
    )?;

    // Eval: Foreign{name:"handle", args:[Child(0), Child(1)]} — handle is
    // complex (effect handlers), use a foreign/host call. Child(0) is the
    // body, Child(1) is the handler clauses collection.
    eval_scheme(
        &mut lang,
        "eval.handle",
        "handle",
        Scheme::foreign(
            "handle",
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
        let e = HandleEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
