//! # handl-emitter-stmt-import
//!
//! Emitter for the **import statement** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-stmt-import` (grammar) into a data-driven
//! [`Language`] fragment. The existing `StmtImportRule` parses an import
//! statement and produces a `"stmt.import"` AST node.
//!
//! ## Grammar
//!
//! A prefix rule for `token.keyword.import` producing a `"stmt.import"` node
//! (arity 1: the source expression).
//!
//! **Delimited form:** import statements require consuming the `import`
//! keyword and a module source expression — the current Pratt engine's
//! grammar row format cannot handle the full delimited syntax. A grammar
//! row is emitted as a **placeholder**. The full import syntax requires a
//! grammar engine that supports delimited forms. The **eval scheme** is the
//! fully migrated part.
//!
//! ## Eval
//!
//! The `"stmt.import"` node kind maps to
//! `Foreign{name:"import", args:[Child(0)]}`. Import is a host capability
//! (module loading): the `import` host function loads the module identified
//! by the source expression (child 0) and binds its exports into the current
//! scope.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `StmtImportRule` produces `AstNode{kind:"stmt.import",
//! payload:{...}}`. The data-driven path uses the same node kind and the
//! `import` host capability performs the module load. Parity is on the
//! **evaluated effect** (the imported bindings become available in scope).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_compound, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the import statement construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct StmtImportEmitter;

impl StmtImportEmitter {
    /// Create a new stmt-import emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for StmtImportEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the import statement construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: `import` keyword.
    declare_token(&mut lang, "token.keyword.import")?;

    // Grammar: prefix rule for `import` keyword (arity 1 — the source
    // expression). Simplified; the Pratt engine cannot handle the full
    // delimited import syntax. This row is a placeholder.
    prefix_compound(
        &mut lang,
        "grammar.stmt-import.prefix",
        "token.keyword.import",
        "stmt.import",
        1,
        0,
    )?;

    // Eval: Foreign{import, [Child(0)]} — import is a host capability
    // (module loading).
    eval_scheme(
        &mut lang,
        "eval.stmt-import",
        "stmt.import",
        Scheme::foreign("import", vec![Scheme::child(0)]),
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
        let e = StmtImportEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
