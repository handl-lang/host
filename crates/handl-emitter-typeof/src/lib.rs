//! # handl-emitter-typeof
//!
//! Emitter for the **typeof** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-typeof` (grammar stub) into a data-driven
//! [`Language`] fragment. The source crate is a 20-line stub — the grammar
//! rule was never implemented in Rust. This emitter creates the data-driven
//! version from scratch based on the language design: `typeof expr` is a
//! prefix operator that returns the type name of an expression as a string.
//!
//! ## Grammar
//!
//! A prefix rule for `token.keyword.typeof` producing a `"typeof"` node
//! (arity 1). The prefix right binding power is 70, so `typeof` binds tighter
//! than the infix arithmetic/comparison operators (BP 55–60) but looser than
//! postfix operators (BP 90). This means `typeof x + 1` parses as
//! `(typeof x) + 1` and `typeof x.field` parses as `typeof(x.field)`.
//!
//! ## Eval
//!
//! The `"typeof"` node kind maps to
//! `Prim{op:"typeof", args:[Child(0)]}`. The tree-walk engine's built-in
//! `typeof` primitive evaluates `Child(0)` and returns the type name of the
//! resulting value as a string (e.g. `"number"`, `"string"`, `"bool"`,
//! `"null"`, `"array"`, `"object"`, `"function"`).
//!
//! ## Parity
//!
//! The existing `plugin-grammar-primary-typeof` is a stub with no Rust rule.
//! The data-driven path is the first implementation. Parity is on the
//! **evaluated value** (e.g. `typeof 42 = "number"`).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the typeof construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypeofEmitter;

impl TypeofEmitter {
    /// Create a new typeof emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for TypeofEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the typeof construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `typeof` keyword (already declared in the tokens emitter;
    // re-declared here so this crate is self-contained).
    declare_token(&mut lang, "token.keyword.typeof")?;

    // Grammar: prefix rule for `typeof` (arity 1, prefix_rbp 70 — binds
    // tighter than infix operators but looser than postfix).
    prefix_compound(
        &mut lang,
        "grammar.typeof.prefix",
        "token.keyword.typeof",
        "typeof",
        1,
        70,
    )?;

    // Eval: Prim{op:"typeof", args:[Child(0)]} — returns the type name as a
    // string.
    eval_scheme(
        &mut lang,
        "eval.typeof",
        "typeof",
        Scheme::prim("typeof", vec![Scheme::child(0)]),
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
        let e = TypeofEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
