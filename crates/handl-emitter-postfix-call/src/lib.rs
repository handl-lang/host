//! # handl-emitter-postfix-call
//!
//! Emitter for the **postfix function call** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-postfix-call` (grammar stub) and
//! `plugin-grammar-call` (125-line real impl) + `plugin-eval-call` (eval)
//! into data-driven [`Language`] fragments.
//!
//! ## Grammar
//!
//! A postfix rule for `token.lparen` (`(`) producing a `"call"` node
//! (arity 2, placeholder). The real `CallRule` is an infix rule that parses
//! `callee(arg1, arg2, ...)` — comma-separated arguments delimited by `)`.
//! The current Pratt engine's grammar row format only supports fixed-arity
//! operators; it cannot handle comma separators or closing delimiters.
//!
//! **Postfix encoding:** `GrammarPosition` has only `Prefix` and `Infix`
//! variants — there is no `Postfix` variant. A postfix operator is encoded
//! as an `Infix` row with right binding power 0 (it does not parse a right
//! operand). The binding power is `(90, 0)` — the highest precedence,
//! tighter than everything. A grammar row is emitted as a **placeholder**
//! (arity 2 = callee + first arg). The full call syntax requires a grammar
//! engine that supports delimited, variable-arity argument lists. The
//! **eval scheme** is the fully migrated part; parity tests construct call
//! ASTs manually.
//!
//! ## Eval
//!
//! The `"call"` node kind maps to
//! `App{callee: Child(0), args: [Child(1)]}`. The tree-walk engine
//! evaluates `Child(0)` to obtain a closure value, evaluates `Child(1)` as
//! the first argument, and applies the closure. For variable-arity calls,
//! the real call eval collects all children 1..N as arguments — the
//! `app_call(num_args)` helper builds the scheme for a fixed arg count. This
//! placeholder uses `app_call(1)` (one argument); a future `Scheme`
//! extension is needed for truly variable-arity calls.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `CallRule` produces `AstNode{kind:"call", payload:{"callee":
//! <ast>, "args":[...]}}`. The data-driven path uses `{"children":[callee_idx,
//! arg1_idx]}`. Both evaluate to the same final `Value`. Parity is on the
//! **evaluated value**.

#![warn(missing_docs)]

use handl_emitters::{app_call, declare_token, eval_scheme, Emitter};
use handl_language::{
    AstShapeRef, GrammarPosition, GrammarRow, LangError, Language,
};

/// Emitter for the postfix function call construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct PostfixCallEmitter;

impl PostfixCallEmitter {
    /// Create a new postfix-call emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for PostfixCallEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the postfix function call construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Tokens: `(`, `)`, `,` (already declared in the tokens emitter;
    // re-declared here so this crate is self-contained).
    declare_token(&mut lang, "token.lparen")?;
    declare_token(&mut lang, "token.rparen")?;
    declare_token(&mut lang, "token.comma")?;

    // Grammar: postfix rule for `(` (arity 2 = callee + first arg,
    // placeholder). Postfix is encoded as Infix with rbp=0 (no right
    // operand parsed). Binding power (90, 0) — highest precedence.
    // The Pratt engine cannot handle comma-separated args or the closing
    // `)` delimiter.
    lang.grammar.merge(
        GrammarRow::new(
            "grammar.call.postfix",
            "token.lparen",
            GrammarPosition::Infix,
            AstShapeRef::new("call", 2),
        )
        .with_binding_power(90, 0),
    )?;

    // Eval: App{callee: Child(0), args: [Child(1)]} — callee is child 0,
    // first arg is child 1. For variable args, the real call collects all
    // children 1..N as args. This placeholder uses app_call(1) (one arg).
    eval_scheme(&mut lang, "eval.call.postfix", "call", app_call(1))?;

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
        let e = PostfixCallEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
