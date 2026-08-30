//! # handl-emitter-branch
//!
//! Emitter for the **conditional branch** construct (ADR-0044 Phase 4).
//!
//! Migrates the `if`/`match` branching semantics into a data-driven
//! [`Language`] fragment.
//!
//! ## Eval
//!
//! The `"if"` node kind maps to
//! `Branch{cond: Child(0), then_: Child(1), else_: Child(2)}`. The tree-walk
//! engine evaluates the condition, then evaluates either the then-branch or
//! the else-branch based on truthiness.
//!
//! ## Grammar simplification
//!
//! The existing `if` grammar (from `plugin-grammar-infix-flow` or similar)
//! uses keyword-delimited syntax (`if (cond) then else`). The current Pratt
//! engine's grammar row format cannot handle keyword-delimited forms. A
//! grammar row is emitted for `token.if` (prefix, arity 3) as a placeholder,
//! but the full `if` syntax requires a grammar engine that supports
//! delimited forms. The **eval scheme** is the fully migrated part; parity
//! tests construct `if` ASTs manually with the payload shape
//! `{"children": [cond_idx, then_idx, else_idx]}`.
//!
//! ## Parity
//!
//! The data-driven `Branch` scheme evaluates the condition and selects the
//! appropriate branch — the same semantics as the existing Rust `if`/`match`
//! eval rules. Parity is on the **evaluated value**.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the conditional branch construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct BranchEmitter;

impl BranchEmitter {
    /// Create a new branch emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for BranchEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the conditional branch construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();
    declare_token(&mut lang, "token.if")?;
    // Grammar: prefix rule for `if` keyword (arity 3 = cond + then + else;
    // simplified — the Pratt engine cannot handle keyword-delimited forms).
    prefix_compound(&mut lang, "grammar.if.prefix", "token.if", "if", 3, 0)?;
    // Eval: Branch{cond: Child(0), then_: Child(1), else_: Child(2)}
    eval_scheme(&mut lang, "eval.if", "if", Scheme::Branch {
        cond: Box::new(Scheme::child(0)),
        then_: Box::new(Scheme::child(1)),
        else_: Box::new(Scheme::child(2)),
    })?;
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
        let e = BranchEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
