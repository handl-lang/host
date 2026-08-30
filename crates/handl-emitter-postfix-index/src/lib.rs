//! # handl-emitter-postfix-index
//!
//! Emitter for the **postfix index access** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-postfix-index` (grammar stub) into a data-driven
//! [`Language`] fragment. The source crate is a 20-line stub — the grammar
//! rule was never implemented in Rust. This emitter creates the data-driven
//! version from scratch based on the language design: `expr[index]` is a
//! postfix operator that indexes into an array or object.
//!
//! ## Grammar
//!
//! A postfix rule for `token.lbracket` (`[`) producing an `"index"` node
//! (arity 2, placeholder). The real index syntax is `expr[index]` — a
//! delimited form with a closing `]`. The current Pratt engine's grammar
//! row format only supports fixed-arity operators; it cannot handle the
//! closing `]` delimiter.
//!
//! **Postfix encoding:** `GrammarPosition` has only `Prefix` and `Infix`
//! variants — there is no `Postfix` variant. A postfix operator is encoded
//! as an `Infix` row with right binding power 0 (it does not parse a right
//! operand). The binding power is `(90, 0)` — the highest precedence,
//! tighter than everything. A grammar row is emitted as a **placeholder**
//! (arity 2 = target + index). The full `expr[index]` syntax requires a
//! grammar engine that supports delimited forms. The **eval scheme** is the
//! fully migrated part; parity tests construct `index` ASTs manually.
//!
//! ## Eval
//!
//! The `"index"` node kind maps to
//! `Prim{op:"index", args:[Child(0), Child(1)]}`. The tree-walk engine's
//! built-in `index` primitive evaluates `Child(0)` (the target array or
//! object) and `Child(1)` (the index key), then performs the lookup.
//!
//! ## Parity
//!
//! The existing `plugin-grammar-postfix-index` is a stub with no Rust rule.
//! The data-driven path is the first implementation. Parity is on the
//! **evaluated value** (e.g. `[1, 2, 3][0] = 1`).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, Emitter};
use handl_language::{
    AstShapeRef, GrammarPosition, GrammarRow, LangError, Language, Scheme,
};

/// Emitter for the postfix index access construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostfixIndexEmitter;

impl PostfixIndexEmitter {
    /// Create a new postfix-index emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for PostfixIndexEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the postfix index access construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Tokens: `[`, `]` (already declared in the tokens emitter; re-declared
    // here so this crate is self-contained).
    declare_token(&mut lang, "token.lbracket")?;
    declare_token(&mut lang, "token.rbracket")?;

    // Grammar: postfix rule for `[` (arity 2 = target + index, placeholder).
    // Postfix is encoded as Infix with rbp=0. Binding power (90, 0) —
    // highest precedence. The Pratt engine cannot handle the closing `]`
    // delimiter.
    lang.grammar.merge(
        GrammarRow::new(
            "grammar.index.postfix",
            "token.lbracket",
            GrammarPosition::Infix,
            AstShapeRef::new("index", 2),
        )
        .with_binding_power(90, 0),
    )?;

    // Eval: Prim{op:"index", args:[Child(0), Child(1)]} — target is child 0,
    // index is child 1.
    eval_scheme(
        &mut lang,
        "eval.index",
        "index",
        Scheme::prim("index", vec![Scheme::child(0), Scheme::child(1)]),
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
        let e = PostfixIndexEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
