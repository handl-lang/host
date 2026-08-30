//! # handl-emitter-textmate
//!
//! Emitter for the **TextMate grammar contribution** capability
//! (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-textmate` into a data-driven [`Language`]
//! fragment. Unlike the other emitters, `plugin-grammar-textmate` is **not**
//! a parser grammar rule — it is the TextMate grammar generation
//! infrastructure (`GrammarRule`, `GrammarContributor`). It is about editor
//! syntax highlighting, not parsing.
//!
//! ## What this emitter does
//!
//! TextMate grammar generation is a separate concern from parsing/eval. The
//! data-driven migration of TextMate rules requires a new fragment type
//! (`TextMateRuleDef`) which does not yet exist in `handl-language`. This
//! emitter therefore emits **only a `CapabilityDecl`** for the `"textmate"`
//! capability — declaring that a language exercises TextMate grammar
//! generation. It does not emit grammar rows or eval schemes.
//!
//! ## Future work
//!
//! Once a `TextMateRuleDef` fragment type is added to `handl-language`, this
//! emitter should be extended to emit the TextMate rule data (scope names,
//! match/begin/end patterns, capture groups) as data fragments. Until then,
//! the capability declaration is the data-driven foothold for TextMate
//! grammar generation.

#![warn(missing_docs)]

use handl_emitters::Emitter;
use handl_language::{CapabilityDecl, LangError, Language};

/// Emitter for the TextMate grammar contribution capability.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextmateEmitter;

impl TextmateEmitter {
    /// Create a new textmate emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for TextmateEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the TextMate capability [`Language`] fragment.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Declare the "textmate" capability — TextMate grammar generation is a
    // host capability. No grammar rows or eval schemes are emitted; the
    // data-driven migration of TextMate rules requires a `TextMateRuleDef`
    // fragment type (future work).
    lang.caps.merge(
        CapabilityDecl::new("textmate")
            .with_description("TextMate grammar generation for editor syntax highlighting"),
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 0);
        assert_eq!(lang.grammar.len(), 0);
        assert_eq!(lang.eval.len(), 0);
        assert_eq!(lang.caps.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = TextmateEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
