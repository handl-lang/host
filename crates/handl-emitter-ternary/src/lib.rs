//! # handl-emitter-ternary
//!
//! Emitter for the **ternary conditional operator** `? :` (ADR-0044 Phase 4).
//!
//! The stub `plugin-grammar-infix-ternary` has no implementation; this
//! emitter is created from scratch. The ternary `cond ? then : else` is a
//! delimited infix form — the `?` token starts the infix operator and the
//! `:` token delimits the then-branch from the else-branch. The current
//! Pratt engine's grammar row format cannot fully handle delimited forms
//! (the same limitation as `if`), so a placeholder grammar row is emitted
//! and the **eval scheme** is the fully migrated part.
//!
//! ## Grammar
//!
//! A placeholder infix grammar row for `?` at binding power `(15, 16)` —
//! left-associative — producing a `ternary` node (arity 3: cond, then,
//! else). The `:` delimiter token is declared but does not get its own
//! grammar row; a grammar engine that supports delimited infix forms is
//! required for full `? :` syntax. Parity tests construct `ternary` ASTs
//! manually with the payload shape `{"children": [cond_idx, then_idx,
//! else_idx]}`.
//!
//! ## Eval
//!
//! - `ternary` → `Branch{cond: Child(0), then_: Child(1), else_: Child(2)}`
//!
//! Same semantics as `if`: the tree-walk engine evaluates the condition,
//! then evaluates either the then-branch or the else-branch based on
//! truthiness.
//!
//! ## Parity
//!
//! The data-driven `Branch` scheme evaluates the condition and selects the
//! appropriate branch — the same semantics as the existing Rust `if`/`match`
//! eval rules and the `branch` emitter. Parity is on the **evaluated
//! value**.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, Emitter};
use handl_language::{
    AstShapeRef, GrammarPosition, GrammarRow, LangError, Language, Scheme,
};

/// Emitter for the ternary conditional operator construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct TernaryEmitter;

impl TernaryEmitter {
    /// Create a new ternary emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for TernaryEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the ternary conditional operator's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kinds: `?` (new) and `:` (already declared by the tokens emitter;
    // re-declared here so this fragment is self-contained).
    declare_token(&mut lang, "token.question")?;
    declare_token(&mut lang, "token.colon")?;

    // Grammar: placeholder infix rule for `?` (arity 3 — cond, then, else).
    // The Pratt engine cannot handle the `:` delimiter; this row is a
    // placeholder. A grammar engine supporting delimited infix forms is
    // required for full `? :` syntax.
    lang.grammar.merge(
        GrammarRow::new(
            "grammar.ternary.infix",
            "token.question",
            GrammarPosition::Infix,
            AstShapeRef::new("ternary", 3),
        )
        .with_binding_power(15, 16),
    )?;

    // Eval: ternary node maps to a Branch scheme — same semantics as `if`.
    eval_scheme(
        &mut lang,
        "eval.ternary",
        "ternary",
        Scheme::Branch {
            cond: Box::new(Scheme::child(0)),
            then_: Box::new(Scheme::child(1)),
            else_: Box::new(Scheme::child(2)),
        },
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 2);
        assert_eq!(lang.grammar.len(), 1);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = TernaryEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
