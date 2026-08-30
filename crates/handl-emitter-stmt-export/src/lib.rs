//! # handl-emitter-stmt-export
//!
//! Emitter for the **export statement** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-stmt-export` (grammar) into a data-driven
//! [`Language`] fragment. The existing `StmtExportRule` parses an export
//! statement and produces a `"stmt.export"` AST node.
//!
//! ## Grammar
//!
//! A prefix rule for `token.keyword.export` producing a `"stmt.export"` node
//! (arity 1: the exported expression).
//!
//! **Delimited form:** export statements require consuming the `export`
//! keyword and an expression to export — the current Pratt engine's grammar
//! row format cannot handle the full delimited syntax. A grammar row is
//! emitted as a **placeholder**. The full export syntax requires a grammar
//! engine that supports delimited forms. The **eval scheme** is the fully
//! migrated part.
//!
//! ## Eval
//!
//! The `"stmt.export"` node kind maps to
//! `Foreign{name:"export", args:[Child(0)]}`. Export is a host capability:
//! the `export` host function registers the exported expression (child 0)
//! in the module's export table.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `StmtExportRule` produces `AstNode{kind:"stmt.export",
//! payload:{...}}`. The data-driven path uses the same node kind and the
//! `export` host capability performs the registration. Parity is on the
//! **evaluated effect** (the value becomes available in the module's
//! exports).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the export statement construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct StmtExportEmitter;

impl StmtExportEmitter {
    /// Create a new stmt-export emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for StmtExportEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the export statement construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `export` keyword.
    declare_token(&mut lang, "token.keyword.export")?;

    // Grammar: prefix rule for `export` keyword (arity 1 — the exported
    // expression). Simplified; the Pratt engine cannot handle the full
    // delimited export syntax. This row is a placeholder.
    prefix_compound(
        &mut lang,
        "grammar.stmt-export.prefix",
        "token.keyword.export",
        "stmt.export",
        1,
        0,
    )?;

    // Eval: Foreign{export, [Child(0)]} — export is a host capability.
    eval_scheme(
        &mut lang,
        "eval.stmt-export",
        "stmt.export",
        Scheme::foreign("export", vec![Scheme::child(0)]),
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
        let e = StmtExportEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
