//! # handl-emitter-not
//!
//! Emitter for the **logical not** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-not` (grammar stub) and the `not` eval
//! semantics into data-driven [`Language`] fragments. The source
//! `plugin-grammar-primary-not` crate is a 20-line stub — the grammar rule was
//! never implemented in Rust. This emitter creates the data-driven version from
//! scratch based on the language design: `!expr` is a prefix unary operator.
//!
//! ## Grammar
//!
//! A prefix rule for `token.bang` (`!`) producing a `"unary.not"` node
//! (arity 1). The prefix right binding power is 70, so `!` binds tighter than
//! the infix arithmetic/comparison operators (which sit at BP 55–60) but
//! looser than postfix operators (BP 90). This means `!x + 1` parses as
//! `(!x) + 1` and `!x.field` parses as `!(x.field)`.
//!
//! ## Eval
//!
//! The `"unary.not"` node kind maps to
//! `Prim{op:"not", args:[Child(0)]}`. The tree-walk engine's built-in `not`
//! primitive evaluates `Child(0)`, checks truthiness, and returns the logical
//! negation.
//!
//! ## Parity
//!
//! The existing `plugin-grammar-primary-not` is a stub with no Rust rule. The
//! data-driven path is the first implementation. Parity is on the
//! **evaluated value** (e.g. `!true = false`).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the logical not construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct NotEmitter;

impl NotEmitter {
    /// Create a new not emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for NotEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the logical not construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `!` (new — not in the tokens emitter).
    declare_token(&mut lang, "token.bang")?;

    // Grammar: prefix rule for `!` (arity 1, prefix_rbp 70 — binds tighter
    // than infix operators but looser than postfix).
    prefix_compound(&mut lang, "grammar.not.prefix", "token.bang", "unary.not", 1, 70)?;

    // Eval: Prim{op:"not", args:[Child(0)]}
    eval_scheme(
        &mut lang,
        "eval.unary.not",
        "unary.not",
        Scheme::prim("not", vec![Scheme::child(0)]),
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
        let e = NotEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
