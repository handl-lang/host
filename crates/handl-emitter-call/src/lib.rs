//! # handl-emitter-call
//!
//! Emitter for the **function call** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-call` (grammar) and `plugin-eval-call` (eval)
//! into data-driven [`Language`] fragments.
//!
//! ## Eval
//!
//! The `"call"` node kind maps to
//! `App{callee: Child(0), args: [Child(1), Child(2)]}`. The tree-walk
//! engine evaluates `Child(0)` to obtain a closure value, evaluates
//! `Child(1)` and `Child(2)` as arguments, and applies the closure.
//!
//! This emitter targets **2-argument calls** (`f(a, b)`). The `Scheme` type
//! has a fixed argument list at emit time; variable-arity calls require
//! either multiple node kinds (`call1`, `call2`, ...) or a future `Scheme`
//! extension. The subset uses 2-arg calls (e.g. `f(1, 2)`).
//!
//! ## Grammar simplification
//!
//! The existing `CallRule` is an infix rule for `(` that parses
//! comma-separated arguments. The current Pratt engine's grammar row format
//! only supports fixed-arity infix operators — it cannot handle comma
//! separators. A grammar row is emitted for `token.lparen` (infix, arity 3)
//! as a placeholder, but the full call syntax requires a grammar engine that
//! supports delimited argument lists. The **eval scheme** is the fully
//! migrated part; parity tests construct call ASTs manually.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `CallRule` produces `AstNode{kind:"call", payload:{"callee":
//! <ast>, "args":[...]}}`. The data-driven path uses `{"children":[callee_idx,
//! arg1_idx, arg2_idx]}`. Both evaluate to the same final `Value`.

#![warn(missing_docs)]

use handl_emitters::{app_call, declare_token, eval_scheme, infix_binary, Emitter};
use handl_language::{LangError, Language};

/// Emitter for the function call construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct CallEmitter;

impl CallEmitter {
    /// Create a new call emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for CallEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the function call construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();
    declare_token(&mut lang, "token.lparen")?;
    declare_token(&mut lang, "token.rparen")?;
    // Grammar: infix rule for `(` (arity 3 = callee + 2 args; simplified —
    // the Pratt engine cannot handle comma-separated args). Binding power
    // (90, 91) matches the existing CallRule.
    infix_binary(&mut lang, "grammar.call.infix", "token.lparen", "call", 90, 91)?;
    // Eval: App{callee: Child(0), args: [Child(1), Child(2)]}
    eval_scheme(&mut lang, "eval.call", "call", app_call(2))?;
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
        let e = CallEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
