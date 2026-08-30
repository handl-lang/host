//! # handl-emitter-array
//!
//! Emitter for the **array literal** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-array` (grammar stub) and
//! `plugin-grammar-array` (115-line real impl) + `plugin-eval-array` (eval)
//! into data-driven [`Language`] fragments.
//!
//! ## Grammar
//!
//! A prefix rule for `token.lbracket` (`[`) producing an `"array"` node
//! (arity 1, placeholder). The real `ArrayRule` parses `[elem1, elem2, ...]`
//! — a delimited form with comma-separated elements and a closing `]`. The
//! current Pratt engine's grammar row format only supports fixed-arity
//! prefix operators; it cannot handle comma separators or closing
//! delimiters. A grammar row is emitted as a **placeholder** (arity 1). The
//! full array syntax requires a grammar engine that supports delimited,
//! variable-arity forms. The **eval scheme** is the fully migrated part;
//! parity tests construct `array` ASTs manually.
//!
//! ## Eval
//!
//! The `"array"` node kind maps to
//! `Prim{op:"array", args:[Child(0)]}`. The tree-walk engine's built-in
//! `array` primitive constructs an array value from its children. The real
//! array eval collects **all** children (0..N) into a JSON array value; the
//! placeholder scheme references `Child(0)` only — a future `Scheme`
//! extension (or variable-arity primitive) is needed for the full semantics.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `ArrayRule` produces `AstNode{kind:"array",
//! payload:{"elements": [...]}}`. The data-driven path uses
//! `{"children": [idx0, ...]}`. Both evaluate to the same final `Value` (a
//! JSON array). Parity is on the **evaluated value**.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the array literal construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArrayEmitter;

impl ArrayEmitter {
    /// Create a new array emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for ArrayEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the array literal construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Tokens: `[`, `]`, `,` (already declared in the tokens emitter;
    // re-declared here so this crate is self-contained).
    declare_token(&mut lang, "token.lbracket")?;
    declare_token(&mut lang, "token.rbracket")?;
    declare_token(&mut lang, "token.comma")?;

    // Grammar: prefix rule for `[` (arity 1, placeholder — the real array
    // has variable elements separated by commas). Delimited form `[a, b, c]`.
    // The Pratt engine cannot handle the closing `]` or comma separators.
    prefix_compound(&mut lang, "grammar.array.prefix", "token.lbracket", "array", 1, 0)?;

    // Eval: Prim{op:"array", args:[Child(0)]} — constructs an array from
    // children. The real array eval collects all children 0..N into a JSON
    // array value; this placeholder references Child(0) only.
    eval_scheme(
        &mut lang,
        "eval.array",
        "array",
        Scheme::prim("array", vec![Scheme::child(0)]),
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 3);
        assert_eq!(lang.grammar.len(), 1);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = ArrayEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
