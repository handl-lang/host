//! # handl-emitter-stmt-let
//!
//! Emitter for the **let statement** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-stmt-let` (grammar) into a data-driven
//! [`Language`] fragment. The existing `StmtLetRule` parses
//! `let name = value;` and produces a `"stmt.let"` AST node.
//!
//! ## Grammar
//!
//! A prefix rule for `token.keyword.let` producing a `"stmt.let"` node
//! (arity 2: value + body). This is the **statement** form, distinct from
//! the expression-level `let` (`handl-emitter-let`).
//!
//! **Delimited form:** `let name = value;` requires consuming the `let`
//! keyword, a name, `=`, an expression, and `;` — the current Pratt engine's
//! grammar row format cannot handle fixed delimiters or store the binding
//! name in the payload. A grammar row is emitted as a **placeholder**. The
//! full `let` statement syntax requires a grammar engine that supports
//! delimited forms. The **eval scheme** is the fully migrated part; parity
//! tests construct `stmt.let` ASTs manually with the correct payload shape.
//!
//! ## Eval
//!
//! The `"stmt.let"` node kind maps to
//! `Bind{name:"$payload", value:Child(0), body:Child(1)}`. The binding name
//! is read from `$payload` (the node's `name` field), the value is child 0,
//! and the body is child 1. This mirrors the expression-level `let` eval
//! scheme but uses the `"stmt.let"` node kind.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `StmtLetRule` produces `AstNode{kind:"stmt.let",
//! payload:{"name":..., "value":<ast>}}`. The data-driven path uses
//! `{"name":..., "children":[...]}`. Both evaluate to the same final
//! `Value` — parity is on the **evaluated value**, not the AST payload shape.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the let statement construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct StmtLetEmitter;

impl StmtLetEmitter {
    /// Create a new stmt-let emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for StmtLetEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the let statement construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `let` keyword.
    declare_token(&mut lang, "token.keyword.let")?;

    // Grammar: prefix rule for `let` keyword (arity 2 — simplified; the
    // Pratt engine cannot handle the `name = value;` delimited form). This
    // row is a placeholder.
    prefix_compound(&mut lang, "grammar.stmt-let.prefix", "token.keyword.let", "stmt.let", 2, 0)?;

    // Eval: Bind{name from payload, value=Child(0), body=Child(1)}.
    eval_scheme(
        &mut lang,
        "eval.stmt-let",
        "stmt.let",
        Scheme::Bind {
            name: "$payload".into(),
            value: Box::new(Scheme::child(0)),
            body: Box::new(Scheme::child(1)),
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
        assert_eq!(lang.tokens.len(), 1);
        assert_eq!(lang.grammar.len(), 1);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = StmtLetEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
