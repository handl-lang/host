//! # handl-emitter-object
//!
//! Emitter for the **object literal** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-object` (grammar stub) and
//! `plugin-grammar-object` (156-line real impl) + `plugin-eval-object` (eval)
//! into data-driven [`Language`] fragments.
//!
//! ## Grammar
//!
//! A prefix rule for `token.lbrace` (`{`) producing an `"object"` node
//! (arity 1, placeholder). The real `ObjectRule` parses
//! `{key: value, ...}` — a delimited form with colon-separated key-value
//! pairs, comma-separated entries, and a closing `}`. The current Pratt
//! engine's grammar row format only supports fixed-arity prefix operators;
//! it cannot handle key-value separators, commas, or closing delimiters. A
//! grammar row is emitted as a **placeholder** (arity 1). The full object
//! syntax requires a grammar engine that supports delimited, variable-arity
//! forms. The **eval scheme** is the fully migrated part; parity tests
//! construct `object` ASTs manually.
//!
//! ## Eval
//!
//! The `"object"` node kind maps to
//! `Prim{op:"object", args:[Child(0)]}`. The tree-walk engine's built-in
//! `object` primitive constructs an object value from its children
//! (key-value pair nodes). The real object eval collects **all** children
//! (0..N) into a JSON object value; the placeholder scheme references
//! `Child(0)` only — a future `Scheme` extension (or variable-arity
//! primitive) is needed for the full semantics.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `ObjectRule` produces `AstNode{kind:"object",
//! payload:{"entries": [{"key":..., "value":...}, ...]}}`. The data-driven
//! path uses `{"children": [idx0, ...]}`. Both evaluate to the same final
//! `Value` (a JSON object). Parity is on the **evaluated value**.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the object literal construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct ObjectEmitter;

impl ObjectEmitter {
    /// Create a new object emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for ObjectEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the object literal construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Tokens: `{`, `}`, `,`, `:` (already declared in the tokens emitter;
    // re-declared here so this crate is self-contained).
    declare_token(&mut lang, "token.lbrace")?;
    declare_token(&mut lang, "token.rbrace")?;
    declare_token(&mut lang, "token.comma")?;
    declare_token(&mut lang, "token.colon")?;

    // Grammar: prefix rule for `{` (arity 1, placeholder — the real object
    // has variable key-value pairs separated by commas). Delimited form
    // `{key: value, ...}`. The Pratt engine cannot handle the closing `}`,
    // commas, or the `:` separator.
    prefix_compound(&mut lang, "grammar.object.prefix", "token.lbrace", "object", 1, 0)?;

    // Eval: Prim{op:"object", args:[Child(0)]} — constructs an object from
    // children (key-value pair nodes). The real object eval collects all
    // children 0..N into a JSON object value; this placeholder references
    // Child(0) only.
    eval_scheme(
        &mut lang,
        "eval.object",
        "object",
        Scheme::prim("object", vec![Scheme::child(0)]),
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 4);
        assert_eq!(lang.grammar.len(), 1);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = ObjectEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
