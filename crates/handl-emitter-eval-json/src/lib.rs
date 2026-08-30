//! # handl-emitter-eval-json
//!
//! Emitter for the **JSON construction** eval construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-eval-json` (eval) into data-driven [`Language`] fragments.
//! The existing Rust `JsonEval` rule parses a JSON literal string from
//! `payload["expr"]` (or joins `payload["args"]` string fragments) and returns
//! the parsed value.
//!
//! ## Eval
//!
//! - `json` → `Foreign{name:"json", args:[]}` — JSON construction is a host
//!   capability that reads `$payload.expr` (or `$payload.args`).
//!
//! ## AST shape
//!
//! - `ast.json` — payload schema `{"expr": <string>, "args": [<string>,...]}`.
//!
//! This construct is **not** covered by any grammar emitter — it is an
//! eval-only rule.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `JsonEval` rule reads `payload["expr"]` (or joins
//! `payload["args"]`) and parses it with `serde_json::from_str`. The
//! data-driven scheme delegates to a `json` host foreign function that
//! performs the same parse. Parity is on the evaluated value (the parsed
//! JSON value).

#![warn(missing_docs)]

use handl_emitters::{eval_scheme, Emitter};
use handl_language::{AstShapeDef, LangError, Language, Scheme};
use serde_json::json;

/// Emitter for the JSON construction eval construct.
///
/// Produces the eval scheme and AST shape for the `json` node kind.
#[derive(Debug, Default, Clone, Copy)]
pub struct EvalJsonEmitter;

impl EvalJsonEmitter {
    /// Create a new eval-json emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for EvalJsonEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the JSON construction construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // --- AST shape ---
    lang.ast.merge(
        AstShapeDef::new("ast.json", "json").with_payload_schema(json!({
            "type": "object",
            "properties": {
                "expr": { "type": "string" },
                "args": { "type": "array" }
            }
        })),
    )?;

    // --- Eval scheme ---
    // json: JSON construction is a host capability reading $payload.expr.
    eval_scheme(&mut lang, "eval.json", "json", Scheme::foreign("json", vec![]))?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.ast.len(), 1);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = EvalJsonEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
