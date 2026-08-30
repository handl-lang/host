//! # handl-emitter-assign
//!
//! Emitter for the **assignment operator** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-assign` (grammar) into a data-driven [`Language`]
//! fragment. The existing Rust rule parses `target = value` into an `assign`
//! node with `{target, value}` payload at binding power `(10, 11)` —
//! left-associative. In the data-driven path we use the core scheme approach
//! (ADR-0044 §4): the `assign` node maps to
//! `Prim{op:"assign", args:[Child(0), Child(1)]}`.
//!
//! ## Grammar
//!
//! Infix rule for `=` at binding power `(10, 11)` — left-associative —
//! matching the existing `AssignRule` precedence (the loosest infix operator).
//! Produces an `assign` node (arity 2): child 0 is the target, child 1 is the
//! value.
//!
//! ## Eval
//!
//! - `assign` → `Prim{op:"assign", args:[Child(0), Child(1)]}`
//!
//! The tree-walk engine's `assign` primitive sets the variable named in
//! Child(0)'s payload to Child(1)'s value.
//!
//! > **Note:** This is a **simplified scheme**. Full assignment with lvalue
//! > patterns (field access, index targets, destructuring) is future work;
//! > the current scheme handles only the simple variable-target case.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `AssignRule` produces an `assign` node with `{target, value}`
//! payload; the eval layer reads the target's name and updates the scope. The
//! data-driven path produces the same `assign` node kind with a `Prim{op:
//! "assign"}` scheme referencing children by index. Parity is on the
//! **evaluated effect** (the variable is updated), not the payload shape.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, infix_binary, prim_binary, Emitter};
use handl_language::{LangError, Language};

/// Emitter for the assignment operator construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct AssignEmitter;

impl AssignEmitter {
    /// Create a new assign emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for AssignEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the assignment operator's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kind for the assignment operator.
    declare_token(&mut lang, "token.equal")?;

    // Grammar: infix rule, left-associative at BP (10, 11).
    infix_binary(&mut lang, "grammar.assign.infix", "token.equal", "assign", 10, 11)?;

    // Eval: assign node maps to a Prim scheme.
    // NOTE: simplified scheme — full lvalue assignment is future work.
    eval_scheme(&mut lang, "eval.assign", "assign", prim_binary("assign"))?;

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
        let e = AssignEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
