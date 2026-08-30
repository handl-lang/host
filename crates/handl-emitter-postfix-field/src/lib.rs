//! # handl-emitter-postfix-field
//!
//! Emitter for the **postfix field access** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-postfix-field` (grammar stub) into a data-driven
//! [`Language`] fragment. The source crate is a 20-line stub — the grammar
//! rule was never implemented in Rust. This emitter creates the data-driven
//! version from scratch based on the language design: `expr.fieldname` is a
//! postfix operator that accesses a field of an object.
//!
//! ## Grammar
//!
//! A postfix rule for `token.dot` (`.`) producing a `"field-access"` node
//! (arity 1). The field name comes from the token text (the identifier after
//! the dot), stored in the node payload.
//!
//! **Postfix encoding:** `GrammarPosition` has only `Prefix` and `Infix`
//! variants — there is no `Postfix` variant. A postfix operator is encoded
//! as an `Infix` row with right binding power 0 (it does not parse a right
//! operand). The binding power is `(90, 0)` — the highest precedence,
//! tighter than everything. The Pratt engine treats `.` as an infix
//! operator; the field name (the token after `.`) is stored in the payload
//! via the atom convention. A grammar row is emitted as a **placeholder**;
//! the full `expr.fieldname` syntax requires a grammar engine that handles
//! postfix field access with a trailing identifier.
//!
//! ## Eval
//!
//! The `"field-access"` node kind maps to
//! `Foreign{name:"field_access", args:[Child(0)]}`. The field name is read
//! from the node payload (the token text after `.`). The tree-walk engine
//! evaluates `Child(0)` to obtain the target object, then looks up the
//! field name in the payload. Field access is a foreign/host call because
//! the field name is dynamic (comes from the payload, not a child node).
//!
//! ## Parity
//!
//! The existing `plugin-grammar-postfix-field` is a stub with no Rust rule.
//! The data-driven path is the first implementation. Parity is on the
//! **evaluated value** (e.g. `{x: 1}.x = 1`).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, Emitter};
use handl_language::{
    AstShapeRef, GrammarPosition, GrammarRow, LangError, Language, Scheme,
};

/// Emitter for the postfix field access construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostfixFieldEmitter;

impl PostfixFieldEmitter {
    /// Create a new postfix-field emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for PostfixFieldEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the postfix field access construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `.` (already declared in the tokens emitter; re-declared here
    // so this crate is self-contained).
    declare_token(&mut lang, "token.dot")?;

    // Grammar: postfix rule for `.` (arity 1 — just the target; the field
    // name comes from the token text/payload). Postfix is encoded as Infix
    // with rbp=0. Binding power (90, 0) — highest precedence.
    lang.grammar.merge(
        GrammarRow::new(
            "grammar.field-access.postfix",
            "token.dot",
            GrammarPosition::Infix,
            AstShapeRef::new("field-access", 1),
        )
        .with_binding_power(90, 0),
    )?;

    // Eval: Foreign{name:"field_access", args:[Child(0)]} — the field name
    // comes from the payload. The tree-walk engine evaluates Child(0) (the
    // target object) and looks up the field name from the node payload.
    eval_scheme(
        &mut lang,
        "eval.field-access",
        "field-access",
        Scheme::foreign("field_access", vec![Scheme::child(0)]),
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
        let e = PostfixFieldEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
