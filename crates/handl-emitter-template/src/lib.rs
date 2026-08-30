//! # handl-emitter-template
//!
//! Emitter for the **template string** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-template` (grammar) into a data-driven
//! [`Language`] fragment. The existing `TemplateRule` is a prefix grammar
//! rule that handles template tokens produced by `plugin-lexer-template`. It
//! produces an AST node of kind `"template"` with a `segments` array
//! containing `{"type":"text","value":"..."}` and
//! `{"type":"expr","raw":"..."}` entries.
//!
//! ## Grammar
//!
//! A prefix rule for `token.template` producing a `"template"` node
//! (arity 0). The template is a single token whose segments are carried in
//! the token payload, so it parses as an atom — no operand expressions are
//! consumed by the grammar row itself.
//!
//! ## Eval
//!
//! The `"template"` node kind maps to `Foreign{name:"eval_template",
//! args:[]}`. Template interpolation requires evaluating embedded
//! expressions, which is a host capability: the `eval_template` host
//! function reads the segment list from `$payload`, concatenates the text
//! segments, and evaluates each `expr` segment in the current scope. This
//! is modeled as a `Foreign` scheme since template interpolation is a host
//! capability that reads segments from the node payload.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `TemplateRule` produces `AstNode{kind:"template",
//! payload:{"segments":[...]}}`. The data-driven path uses the same node
//! kind and the `eval_template` host capability produces the concatenated
//! string value. Parity is on the **evaluated value** (e.g. `` `hello, ${x}` ``
//! with `x = "world"` evaluates to `"hello, world"`).

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, prefix_atom, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the template string construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct TemplateEmitter;

impl TemplateEmitter {
    /// Create a new template emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for TemplateEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the template string construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token: template literal token (new — carries segments in its payload).
    declare_token(&mut lang, "token.template")?;

    // Grammar: prefix atom (arity 0) — the template token carries all
    // segments in its payload, so no operand expressions are parsed.
    prefix_atom(&mut lang, "grammar.template.prefix", "token.template", "template")?;

    // Eval: Foreign{eval_template, []} — template interpolation is a host
    // capability that reads segments from $payload.
    eval_scheme(
        &mut lang,
        "eval.template",
        "template",
        Scheme::foreign("eval_template", vec![]),
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
        let e = TemplateEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
