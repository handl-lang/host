//! # handl-emitter-eval-core
//!
//! Emitter for the **eval kernel core primitives** construct (ADR-0044
//! Phase 4).
//!
//! Migrates `plugin-eval-core` (eval) into data-driven [`Language`] fragments.
//! The existing Rust crate contributes three built-in evaluation primitives
//! that every Handl expression evaluation depends on:
//!
//! - **`var`** — variable reference lookup. Payload: `{"name": "x"}`. The
//!   tree-walk engine reads the variable name from `$payload.name` and looks
//!   it up in the scope stack.
//! - **`resolve`** — dotted path resolution. Payload: `{"path": "foo.bar.baz"}`.
//!   Path resolution is a host capability that reads `$payload.path`.
//! - **`apply`** — function application. Payload: `{"callee": <ast>, "args":
//!   [<ast>,...]}`. Child 0 is the callee, children 1..N are the arguments.
//!
//! These node kinds are **not** covered by any grammar emitter — they are
//! core eval primitives produced by desugaring or constructed directly.
//!
//! ## Eval
//!
//! - `resolve` → `Foreign{name:"resolve", args:[]}` — path resolution is a
//!   host capability that reads `$payload.path`.
//!
//! The `var` and `apply` eval schemes (`eval.var`, `eval.apply`) are **not**
//! emitted by this crate in the canonical kit composition. They share the
//! `var` / `apply` node kinds (and `eval.var` / `eval.apply` fragment ids)
//! with `handl-emitter-identifier` (`eval.var`, atom convention
//! `Var("$payload")`) and `handl-emitter-apply` (`eval.apply`,
//! `Child(0)`). Because `EvalSchemeDef` is a non-union (behavioral) type, two
//! schemes for the same node kind conflict. The canonical kit selects
//! `identifier` and `apply` as the canonical providers of the `var` and
//! `apply` eval schemes; this crate therefore contributes only its **unique**
//! `resolve` primitive (plus the AST shapes, which are union/idempotent and
//! safe to re-declare). A kit that prefers the `plugin-eval-core` `var`/`apply`
//! conventions should include this crate *instead of* `identifier`/`apply`.
//!
//! ## AST shapes
//!
//! - `ast.var` — payload schema `{"name": <string>}`.
//! - `ast.resolve` — payload schema `{"path": <string>}`.
//! - `ast.apply` — payload schema `{"callee": {}, "args": [<ast>,...]}`.
//!
//! These AST shapes are union (idempotent) fragments: re-declaring a shape
//! for a node kind already declared by `handl-emitter-tokens` is a no-op
//! (first declaration wins), never a collision.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `Var`/`Resolve`/`Apply` rules read `payload["name"]`,
//! `payload["path"]`, and `payload["func"]`/`payload["args"]` respectively.
//! The data-driven schemes encode the same reads via the `$payload` convention
//! and `Child(n)` references. Parity is on the evaluated value.

#![warn(missing_docs)]

use handl_emitters::{eval_scheme, Emitter};
use handl_language::{AstShapeDef, LangError, Language, Scheme};
use serde_json::json;

/// Emitter for the eval kernel core primitives construct.
///
/// Produces eval schemes and AST shapes for the `var`, `resolve`, and `apply`
/// node kinds — the three core primitives from `plugin-eval-core`.
#[derive(Debug, Default, Clone, Copy)]
pub struct EvalCoreEmitter;

impl EvalCoreEmitter {
    /// Create a new eval-core emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for EvalCoreEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the eval core primitives' [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // --- AST shapes ---
    lang.ast.merge(
        AstShapeDef::new("ast.var", "var").with_payload_schema(json!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        })),
    )?;
    lang.ast.merge(
        AstShapeDef::new("ast.resolve", "resolve").with_payload_schema(json!({
            "type": "object",
            "properties": { "path": { "type": "string" } }
        })),
    )?;
    lang.ast.merge(
        AstShapeDef::new("ast.apply", "apply").with_payload_schema(json!({
            "type": "object",
            "properties": {
                "callee": {},
                "args": { "type": "array" }
            }
        })),
    )?;

    // --- Eval schemes ---
    // resolve: dotted path resolution — a host capability reading $payload.path.
    // (The `var` and `apply` eval schemes are intentionally NOT emitted here —
    // see the crate-level docs: `identifier` and `apply` are the canonical
    // providers of the `var` / `apply` eval schemes in the canonical kit, and
    // `EvalSchemeDef` is non-union so two schemes for the same node kind
    // conflict. This crate contributes only its unique `resolve` primitive.)
    eval_scheme(
        &mut lang,
        "eval.resolve",
        "resolve",
        Scheme::foreign("resolve", vec![]),
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.ast.len(), 3);
        // Only the unique `resolve` eval scheme is emitted; `var` and `apply`
        // are provided by `identifier` and `apply` in the canonical kit.
        assert_eq!(lang.eval.len(), 1);
        assert!(lang.eval.get_by_id("eval.resolve").is_some());
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = EvalCoreEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
