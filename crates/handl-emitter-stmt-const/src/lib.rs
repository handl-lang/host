//! # handl-emitter-stmt-const
//!
//! Emitter for the **const statement** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-stmt-const` (grammar) into a data-driven
//! [`Language`] fragment. The existing `StmtConstRule` parses
//! `const name = value;` and produces a `"stmt.const"` AST node.
//!
//! ## Note: `const` is deprecated
//!
//! In handl, all bindings are immutable, so `const` is **deprecated** and
//! semantically identical to `let`. This emitter is kept for backward
//! compatibility with the existing crate naming and source migration
//! traceability.
//!
//! ## Grammar
//!
//! A prefix rule for `token.keyword.const` producing a `"stmt.const"` node
//! (arity 2: value + body).
//!
//! **Delimited form:** `const name = value;` requires consuming the `const`
//! keyword, a name, `=`, an expression, and `;` — the current Pratt engine's
//! grammar row format cannot handle fixed delimiters. A grammar row is
//! emitted as a **placeholder**. The full `const` statement syntax requires
//! a grammar engine that supports delimited forms. The **eval scheme** is
//! the fully migrated part.
//!
//! ## Eval
//!
//! The `"stmt.const"` node kind maps to
//! `Bind{name:"$payload", value:Child(0), body:Child(1)}` — the same
//! semantics as `let` (const is semantically identical in handl since all
//! bindings are immutable).
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `StmtConstRule` produces `AstNode{kind:"stmt.const",
//! payload:{"name":..., "value":<ast>}}`. The data-driven path uses
//! `{"name":..., "children":[...]}`. Both evaluate to the same final
//! `Value` — parity is on the **evaluated value**.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the const statement construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct StmtConstEmitter;

impl StmtConstEmitter {
    /// Create a new stmt-const emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for StmtConstEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the const statement construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `const` keyword.
    declare_token(&mut lang, "token.keyword.const")?;

    // Grammar: prefix rule for `const` keyword (arity 2 — simplified; the
    // Pratt engine cannot handle the `name = value;` delimited form). This
    // row is a placeholder.
    prefix_compound(
        &mut lang,
        "grammar.stmt-const.prefix",
        "token.keyword.const",
        "stmt.const",
        2,
        0,
    )?;

    // Eval: Bind{name from payload, value=Child(0), body=Child(1)} — same as
    // let (const is semantically identical in handl).
    eval_scheme(
        &mut lang,
        "eval.stmt-const",
        "stmt.const",
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
        let e = StmtConstEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
