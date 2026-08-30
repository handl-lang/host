//! # handl-emitter-let
//!
//! Emitter for the **let binding** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-let` (grammar) and the binding semantics from
//! `plugin-eval-assign` (eval) into data-driven [`Language`] fragments.
//!
//! ## Eval
//!
//! The `"let"` node kind maps to
//! `Bind{name:"$payload", value:Child(0), body:Child(1)}`. The tree-walk's
//! `$payload` convention reads the binding name from `node.payload["name"]`,
//! and `Child(0)`/`Child(1)` read the value and body from
//! `node.payload["children"][0]` and `[1]` respectively.
//!
//! The node payload must be `{"name": <string>, "children": [value_idx,
//! body_idx]}`.
//!
//! ## Grammar simplification
//!
//! The existing `LetRule` parses `let name = value;` by manually consuming
//! the `let` keyword, name, `=`, expression, and `;` tokens. The current
//! Pratt engine's grammar row format only supports prefix operators that
//! parse N operand expressions — it cannot consume fixed delimiters like `=`
//! and `;` or store the binding name in the payload. A grammar row is emitted
//! for the `let` keyword (prefix, arity 2) as a placeholder, but the full
//! `let` syntax requires a future grammar engine that supports delimited
//! forms. The **eval scheme** is the fully migrated part; parity tests
//! construct `let` ASTs manually with the correct payload shape.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `LetRule` produces `AstNode{kind:"let", payload:{"name":...,
//! "value":<ast>}}`. The data-driven path uses `{"name":..., "children":[...]}`.
//! Both evaluate to the same final `Value` — the parity test compares
//! evaluated values, not AST payload shapes.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the let binding construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct LetEmitter;

impl LetEmitter {
    /// Create a new let emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for LetEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the let binding construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();
    declare_token(&mut lang, "token.let")?;
    // Grammar: prefix rule for `let` keyword (arity 2 — simplified; see
    // crate docs). The full `let name = value;` syntax requires a grammar
    // engine that supports delimited forms. This row is a placeholder.
    prefix_compound(&mut lang, "grammar.let.prefix", "token.let", "let", 2, 0)?;
    // Eval: Bind{name from payload, value=Child(0), body=Child(1)}
    eval_scheme(&mut lang, "eval.let", "let", Scheme::Bind {
        name: "$payload".into(),
        value: Box::new(Scheme::child(0)),
        body: Box::new(Scheme::child(1)),
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
        let e = LetEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
