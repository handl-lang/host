//! # handl-emitter-arith
//!
//! Emitter for the **arithmetic operators** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-arith` (grammar) into data-driven [`Language`]
//! fragments. The existing Rust rule desugars `a + b` into a `call` node with
//! a `math.add` callee; in the data-driven path we use the core scheme
//! approach (ADR-0044 §4 "small core + desugar"): each operator produces a
//! `binary.<op>` node with a `Prim{op, args:[Child(0), Child(1)]}` eval
//! scheme. The tree-walk engine has built-in `add`/`sub`/`mul`/`div`
//! primitives that produce the same evaluated values.
//!
//! ## Grammar
//!
//! Infix rules for `+`, `-` (binding power `(55, 56)`) and `*`, `/`
//! (binding power `(60, 61)`), matching the existing `ArithRule` precedence.
//! Each produces a `binary.<op>` node (arity 2).
//!
//! ## Eval
//!
//! - `binary.add` → `Prim{op:"add", args:[Child(0), Child(1)]}`
//! - `binary.sub` → `Prim{op:"sub", args:[Child(0), Child(1)]}`
//! - `binary.mul` → `Prim{op:"mul", args:[Child(0), Child(1)]}`
//! - `binary.div` → `Prim{op:"div", args:[Child(0), Child(1)]}`
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `ArithRule` desugars `a + b` to `call(math.add(a, b))` at
//! parse time; eval calls the `math.add` host function. The data-driven path
//! produces `binary.add` with a `Prim{op:"add"}` scheme; the tree-walk
//! engine's built-in `add` primitive computes the same result. Parity is on
//! the **evaluated value** (e.g. `1 + 2 = 3`), not the AST node kind — the
//! data path uses a different desugaring strategy (core scheme vs.
//! module-qualified call), which is expected.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, infix_binary, prim_binary, Emitter};
use handl_language::{LangError, Language};

/// Emitter for the arithmetic operators construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArithEmitter;

impl ArithEmitter {
    /// Create a new arith emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for ArithEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the arithmetic operators' [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kinds for the five arithmetic operators.
    declare_token(&mut lang, "token.plus")?;
    declare_token(&mut lang, "token.minus")?;
    declare_token(&mut lang, "token.star")?;
    declare_token(&mut lang, "token.slash")?;
    declare_token(&mut lang, "token.pow")?;

    // Grammar: infix rules with the same binding powers as ArithRule.
    // +/- : (55, 56) — left-associative
    infix_binary(&mut lang, "grammar.add.infix", "token.plus", "binary.add", 55, 56)?;
    infix_binary(&mut lang, "grammar.sub.infix", "token.minus", "binary.sub", 55, 56)?;
    // */ : (60, 61) — left-associative, binds tighter than +/-
    infix_binary(&mut lang, "grammar.mul.infix", "token.star", "binary.mul", 60, 61)?;
    infix_binary(&mut lang, "grammar.div.infix", "token.slash", "binary.div", 60, 61)?;
    // ** : (65, 64) — right-associative, binds tighter than */
    infix_binary(&mut lang, "grammar.pow.infix", "token.pow", "binary.pow", 65, 64)?;

    // Eval: each binary node maps to a Prim scheme.
    eval_scheme(&mut lang, "eval.binary.add", "binary.add", prim_binary("add"))?;
    eval_scheme(&mut lang, "eval.binary.sub", "binary.sub", prim_binary("sub"))?;
    eval_scheme(&mut lang, "eval.binary.mul", "binary.mul", prim_binary("mul"))?;
    eval_scheme(&mut lang, "eval.binary.div", "binary.div", prim_binary("div"))?;
    eval_scheme(&mut lang, "eval.binary.pow", "binary.pow", prim_binary("pow"))?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 5);
        assert_eq!(lang.grammar.len(), 5);
        assert_eq!(lang.eval.len(), 5);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = ArithEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
