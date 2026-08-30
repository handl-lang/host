//! # handl-emitter-match
//!
//! Emitter for the **match expression** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-primary-match` (grammar stub) and
//! `plugin-grammar-match` (181-line real impl) + `plugin-eval-match` (eval)
//! into data-driven [`Language`] fragments.
//!
//! ## Grammar
//!
//! A prefix rule for `token.keyword.match` producing a `"match"` node
//! (arity 2, placeholder). The real `MatchRule` parses
//! `match subject { pattern: body, ... }` — a delimited form with a
//! brace-enclosed arm block, colon-separated pattern-body pairs, and
//! comma-separated arms. The current Pratt engine's grammar row format only
//! supports fixed-arity prefix operators; it cannot handle the brace
//! delimiters, colons, or commas. A grammar row is emitted as a
//! **placeholder** (arity 2 = scrutinee + arms). The full `match` syntax
//! requires a grammar engine that supports delimited, variable-arity forms.
//! The **eval scheme** is the fully migrated part; parity tests construct
//! `match` ASTs manually.
//!
//! ## Eval
//!
//! The `"match"` node kind maps to
//! `Foreign{name:"match", args:[Child(0), Child(1)]}`. Match is a complex
//! construct (pattern matching with guards), so it uses a foreign (host)
//! call. `Child(0)` is the scrutinee and `Child(1)` is the arms collection.
//! Full pattern-matching eval is a foreign/host capability — the host
//! runtime implements the `match` foreign function, which evaluates the
//! scrutinee, tests each arm's pattern against the result, and evaluates
//! the first matching arm's body.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `MatchRule` produces `AstNode{kind:"match",
//! payload:{"subject": <ast>, "arms": [...]}}`. The data-driven path uses
//! `{"children": [scrutinee_idx, arms_idx]}`. Both evaluate to the same
//! final `Value`. Parity is on the **evaluated value**.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the match expression construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct MatchEmitter;

impl MatchEmitter {
    /// Create a new match emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for MatchEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the match expression construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Tokens: `match` keyword, `{`, `}`, `,`, `:` (already declared in the
    // tokens emitter; re-declared here so this crate is self-contained).
    declare_token(&mut lang, "token.keyword.match")?;
    declare_token(&mut lang, "token.lbrace")?;
    declare_token(&mut lang, "token.rbrace")?;
    declare_token(&mut lang, "token.comma")?;
    declare_token(&mut lang, "token.colon")?;

    // Grammar: prefix rule for `match` (arity 2 = scrutinee + arms,
    // placeholder). Delimited form `match subject { arms }`. The Pratt
    // engine cannot handle the brace delimiters, colons, or commas.
    prefix_compound(
        &mut lang,
        "grammar.match.prefix",
        "token.keyword.match",
        "match",
        2,
        0,
    )?;

    // Eval: Foreign{name:"match", args:[Child(0), Child(1)]} — match is
    // complex (pattern matching), use a foreign/host call. Child(0) is the
    // scrutinee, Child(1) is the arms collection.
    eval_scheme(
        &mut lang,
        "eval.match",
        "match",
        Scheme::foreign(
            "match",
            vec![Scheme::child(0), Scheme::child(1)],
        ),
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 5);
        assert_eq!(lang.grammar.len(), 1);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = MatchEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
