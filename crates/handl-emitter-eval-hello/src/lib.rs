//! # handl-emitter-eval-hello
//!
//! Emitter for the **`hello.greet`** demo/test eval construct (ADR-0044
//! Phase 4).
//!
//! Migrates `plugin-eval-hello` (eval) into data-driven [`Language`] fragments.
//! The existing Rust `HelloGreet` rule handles `hello.greet(...)` expressions:
//! it reads the name from `payload["name"]` (or parses it from the call args)
//! and returns `greet(name)` as a string.
//!
//! ## Eval
//!
//! - `hello.greet` → `Foreign{name:"hello.greet", args:[]}` — a demo foreign
//!   function. The tree-walk engine reads the name from `$payload.name`.
//!
//! ## AST shape
//!
//! - `ast.hello.greet` — payload schema `{"name": <string>}`.
//!
//! This construct is **not** covered by any grammar emitter — it is a
//! demo/test eval rule.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `HelloGreet` rule reads `payload["name"]` and calls
//! `plugin-lib-hello::greet`. The data-driven scheme delegates to a
//! `hello.greet` host foreign function that performs the same greeting.
//! Parity is on the evaluated value (the greeting string).

#![warn(missing_docs)]

use handl_emitters::{eval_scheme, Emitter};
use handl_language::{AstShapeDef, LangError, Language, Scheme};
use serde_json::json;

/// Emitter for the `hello.greet` demo/test eval construct.
///
/// Produces the eval scheme and AST shape for the `hello.greet` node kind.
#[derive(Debug, Default, Clone, Copy)]
pub struct EvalHelloEmitter;

impl EvalHelloEmitter {
    /// Create a new eval-hello emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for EvalHelloEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the `hello.greet` construct's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // --- AST shape ---
    lang.ast.merge(
        AstShapeDef::new("ast.hello.greet", "hello.greet").with_payload_schema(json!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        })),
    )?;

    // --- Eval scheme ---
    // hello.greet: a demo foreign function — reads the name from $payload.name.
    eval_scheme(
        &mut lang,
        "eval.hello.greet",
        "hello.greet",
        Scheme::foreign("hello.greet", vec![]),
    )?;

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
        let e = EvalHelloEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
