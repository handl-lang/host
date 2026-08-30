//! # handl-emitter-cmp
//!
//! Emitter for the **comparison operators** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-cmp` (grammar) into data-driven [`Language`]
//! fragments. The existing Rust rule desugars `a == b` into a `call` node with
//! a `cmp.eq` callee; in the data-driven path we use the core scheme approach
//! (ADR-0044 §4 "small core + desugar"): each operator produces a
//! `binary.<op>` node with a `Prim{op, args:[Child(0), Child(1)]}` eval scheme.
//! The tree-walk engine has built-in `eq`/`neq`/`lt`/`lte`/`gt`/`gte`/`compare`
//! primitives that produce the same evaluated values.
//!
//! ## Grammar
//!
//! Infix rules for `==`, `!=`, `<`, `<=`, `>`, `>=`, and `<=>` (spaceship),
//! all at binding power `(50, 51)` — left-associative — matching the existing
//! `CmpRule` precedence (below postfix call/index at 90 and above arrow fanout
//! `&&&` at 25, pipe `|>` at 30, and arrow composition `>>>` at 20). Each
//! produces a `binary.<op>` node (arity 2).
//!
//! ## Eval
//!
//! - `binary.eq` → `Prim{op:"eq", args:[Child(0), Child(1)]}`
//! - `binary.neq` → `Prim{op:"neq", args:[Child(0), Child(1)]}`
//! - `binary.lt` → `Prim{op:"lt", args:[Child(0), Child(1)]}`
//! - `binary.lte` → `Prim{op:"lte", args:[Child(0), Child(1)]}`
//! - `binary.gt` → `Prim{op:"gt", args:[Child(0), Child(1)]}`
//! - `binary.gte` → `Prim{op:"gte", args:[Child(0), Child(1)]}`
//! - `binary.compare` → `Prim{op:"compare", args:[Child(0), Child(1)]}`
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `CmpRule` desugars `a == b` to `call(cmp.eq(a, b))` at parse
//! time; eval calls the `cmp.eq` host function. The data-driven path produces
//! `binary.eq` with a `Prim{op:"eq"}` scheme; the tree-walk engine's built-in
//! `eq` primitive computes the same result. Parity is on the **evaluated
//! value** (e.g. `1 == 1 = true`), not the AST node kind — the data path uses
//! a different desugaring strategy (core scheme vs. module-qualified call),
//! which is expected.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, infix_binary, prim_binary, Emitter};
use handl_language::{LangError, Language};

/// Emitter for the comparison operators construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct CmpEmitter;

impl CmpEmitter {
    /// Create a new cmp emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for CmpEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the comparison operators' [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kinds for the seven comparison operators.
    declare_token(&mut lang, "token.eqeq")?;
    declare_token(&mut lang, "token.neq")?;
    declare_token(&mut lang, "token.lt")?;
    declare_token(&mut lang, "token.lte")?;
    declare_token(&mut lang, "token.gt")?;
    declare_token(&mut lang, "token.gte")?;
    declare_token(&mut lang, "token.spaceship")?;

    // Grammar: infix rules, all left-associative at BP (50, 51).
    infix_binary(&mut lang, "grammar.eq.infix", "token.eqeq", "binary.eq", 50, 51)?;
    infix_binary(&mut lang, "grammar.neq.infix", "token.neq", "binary.neq", 50, 51)?;
    infix_binary(&mut lang, "grammar.lt.infix", "token.lt", "binary.lt", 50, 51)?;
    infix_binary(&mut lang, "grammar.lte.infix", "token.lte", "binary.lte", 50, 51)?;
    infix_binary(&mut lang, "grammar.gt.infix", "token.gt", "binary.gt", 50, 51)?;
    infix_binary(&mut lang, "grammar.gte.infix", "token.gte", "binary.gte", 50, 51)?;
    infix_binary(
        &mut lang,
        "grammar.compare.infix",
        "token.spaceship",
        "binary.compare",
        50,
        51,
    )?;

    // Eval: each binary node maps to a Prim scheme.
    eval_scheme(&mut lang, "eval.binary.eq", "binary.eq", prim_binary("eq"))?;
    eval_scheme(&mut lang, "eval.binary.neq", "binary.neq", prim_binary("neq"))?;
    eval_scheme(&mut lang, "eval.binary.lt", "binary.lt", prim_binary("lt"))?;
    eval_scheme(&mut lang, "eval.binary.lte", "binary.lte", prim_binary("lte"))?;
    eval_scheme(&mut lang, "eval.binary.gt", "binary.gt", prim_binary("gt"))?;
    eval_scheme(&mut lang, "eval.binary.gte", "binary.gte", prim_binary("gte"))?;
    eval_scheme(
        &mut lang,
        "eval.binary.compare",
        "binary.compare",
        prim_binary("compare"),
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 7);
        assert_eq!(lang.grammar.len(), 7);
        assert_eq!(lang.eval.len(), 7);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = CmpEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
