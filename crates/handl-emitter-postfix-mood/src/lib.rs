//! # handl-emitter-postfix-mood
//!
//! Emitter for the **postfix mood** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-postfix-mood` (grammar stub) into a data-driven
//! [`Language`] fragment. The source crate is a 20-line stub with no details
//! — the grammar rule was never implemented in Rust. This emitter creates a
//! minimal placeholder based on the language design: a mood suffix operator
//! (e.g. `expr!` or `expr?`) that annotates an expression with a type/effect
//! mood.
//!
//! ## Grammar
//!
//! A postfix rule for `token.mood` producing a `"mood"` node (arity 1,
//! placeholder). The mood operator is a suffix that attaches to an
//! expression. The exact mood token (e.g. `!` or `?`) and its semantics are
//! not yet specified in the language design — this is a minimal placeholder.
//!
//! **Postfix encoding:** `GrammarPosition` has only `Prefix` and `Infix`
//! variants — there is no `Postfix` variant. A postfix operator is encoded
//! as an `Infix` row with right binding power 0 (it does not parse a right
//! operand). The binding power is `(90, 0)` — the highest precedence,
//! tighter than everything. A grammar row is emitted as a **placeholder**;
//! the full mood syntax requires a grammar engine that supports postfix
//! suffix operators.
//!
//! ## Eval
//!
//! The `"mood"` node kind maps to `Child(0)` — a mood is a type/effect
//! annotation, so eval passes through to the inner expression. The mood
//! suffix does not change the evaluated value; it only affects type-checking
//! or effect tracking (a host/typechecker concern, not an eval concern).
//!
//! ## Parity
//!
//! The existing `plugin-grammar-postfix-mood` is a stub with no Rust rule.
//! The data-driven path is the first implementation (a minimal placeholder).
//! Parity is on the **evaluated value** (mood is a pass-through at eval
//! time).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, Emitter};
use handl_language::{
    AstShapeRef, GrammarPosition, GrammarRow, LangError, Language, Scheme,
};

/// Emitter for the postfix mood construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostfixMoodEmitter;

impl PostfixMoodEmitter {
    /// Create a new postfix-mood emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for PostfixMoodEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the postfix mood construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `mood` (new — not in the tokens emitter). The exact mood token
    // (e.g. `!` or `?`) is not yet specified; this is a placeholder.
    declare_token(&mut lang, "token.mood")?;

    // Grammar: postfix rule for mood (arity 1, placeholder). Postfix is
    // encoded as Infix with rbp=0. Binding power (90, 0) — highest
    // precedence.
    lang.grammar.merge(
        GrammarRow::new(
            "grammar.mood.postfix",
            "token.mood",
            GrammarPosition::Infix,
            AstShapeRef::new("mood", 1),
        )
        .with_binding_power(90, 0),
    )?;

    // Eval: Child(0) — mood is a type/effect annotation, eval passes
    // through to the inner expression.
    eval_scheme(&mut lang, "eval.mood", "mood", Scheme::child(0))?;

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
        let e = PostfixMoodEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
