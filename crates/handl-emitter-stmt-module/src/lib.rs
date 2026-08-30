//! # handl-emitter-stmt-module
//!
//! Emitter for the **module statement** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-stmt-module` (grammar) into a data-driven
//! [`Language`] fragment. The existing `StmtModuleRule` parses a module
//! statement and produces a `"stmt.module"` AST node.
//!
//! ## Grammar
//!
//! A prefix rule for `token.keyword.module` producing a `"stmt.module"` node
//! (arity 2: name + body).
//!
//! **Delimited form:** module statements require consuming the `module`
//! keyword, a name, and a body block — the current Pratt engine's grammar
//! row format cannot handle the full delimited syntax. A grammar row is
//! emitted as a **placeholder**. The full module syntax requires a grammar
//! engine that supports delimited forms. The **eval scheme** is the fully
//! migrated part.
//!
//! ## Eval
//!
//! The `"stmt.module"` node kind maps to
//! `Foreign{name:"module", args:[Child(0), Child(1)]}`. Module is a host
//! capability (namespace creation): the `module` host function creates a
//! namespace from the name (child 0) and body (child 1), evaluating the
//! body in a fresh scope and collecting its bindings into a module value.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `StmtModuleRule` produces `AstNode{kind:"stmt.module",
//! payload:{...}}`. The data-driven path uses the same node kind and the
//! `module` host capability performs the namespace creation. Parity is on
//! the **evaluated value** (a module/namespace value containing the body's
//! bindings).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the module statement construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct StmtModuleEmitter;

impl StmtModuleEmitter {
    /// Create a new stmt-module emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for StmtModuleEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the module statement construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `module` keyword.
    declare_token(&mut lang, "token.keyword.module")?;

    // Grammar: prefix rule for `module` keyword (arity 2 — name + body).
    // Simplified; the Pratt engine cannot handle the full delimited module
    // syntax. This row is a placeholder.
    prefix_compound(
        &mut lang,
        "grammar.stmt-module.prefix",
        "token.keyword.module",
        "stmt.module",
        2,
        0,
    )?;

    // Eval: Foreign{module, [Child(0), Child(1)]} — module is a host
    // capability (namespace creation).
    eval_scheme(
        &mut lang,
        "eval.stmt-module",
        "stmt.module",
        Scheme::foreign("module", vec![Scheme::child(0), Scheme::child(1)]),
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
        let e = StmtModuleEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
