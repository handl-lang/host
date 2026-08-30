//! # handl-emitter-type-annot
//!
//! Emitter for the **type annotation** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-type-annot` (grammar) into a data-driven
//! [`Language`] fragment. The existing `TypeAnnotRule` parses
//! `name: type = value` — an infix rule triggered by the `:` delimiter. It
//! parses the type name (an identifier), expects `=`, then parses the value
//! expression, producing a `stmt.let` AST node with an optional `type` field.
//!
//! ## Grammar
//!
//! An infix rule for `token.colon` (`:`) producing a `"type-annot"` node
//! (arity 2: name + value). Binding power `(12, 13)` — left-associative,
//! looser than arithmetic. The `"type-annot"` node kind is a distinct kind
//! that desugars to a let binding.
//!
//! **Delimited form:** `name: type = value` requires consuming a type
//! expression between `:` and `=` — the current Pratt engine's grammar row
//! format cannot handle this. A grammar row is emitted as a **placeholder**.
//! The full type-annotation syntax requires a grammar engine that supports
//! delimited forms. The **eval scheme** is the fully migrated part; parity
//! tests construct `type-annot` ASTs manually.
//!
//! ## Eval
//!
//! The `"type-annot"` node kind maps to
//! `Bind{name:"$payload", value:Child(1), body:Child(0)}`. The type
//! annotation is **erased at eval time** — it is a typechecker concern, not
//! a runtime concern. The value (child 1) is bound to the name (read from
//! `$payload`), and the body (child 0) is evaluated in that scope. This
//! mirrors the `let` binding semantics: the type annotation is ignored at
//! eval time.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `TypeAnnotRule` produces `AstNode{kind:"stmt.let",
//! payload:{"name":..., "type":..., "value":<ast>}}`. The data-driven path
//! uses a distinct `"type-annot"` node kind (which desugars to let) and
//! erases the type at eval time. Both evaluate to the same final `Value` —
//! parity is on the **evaluated value**, not the AST node kind.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, infix_binary, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the type annotation construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypeAnnotEmitter;

impl TypeAnnotEmitter {
    /// Create a new type-annot emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for TypeAnnotEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the type annotation construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `:` (colon delimiter, infix position).
    declare_token(&mut lang, "token.colon")?;

    // Grammar: infix rule for `:` (arity 2 — simplified; the Pratt engine
    // cannot handle the type-expression-then-`=` delimited form). Binding
    // power (12, 13). This row is a placeholder.
    infix_binary(&mut lang, "grammar.type-annot.infix", "token.colon", "type-annot", 12, 13)?;

    // Eval: Bind{name from payload, value=Child(1), body=Child(0)}. The type
    // annotation is erased at eval time — it is a typechecker concern.
    eval_scheme(
        &mut lang,
        "eval.type-annot",
        "type-annot",
        Scheme::Bind {
            name: "$payload".into(),
            value: Box::new(Scheme::child(1)),
            body: Box::new(Scheme::child(0)),
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
        let e = TypeAnnotEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
