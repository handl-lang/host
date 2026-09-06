//! Data-driven AST desugar pass — thin delegate over the generic
//! transform engine (ADR-0094 §6).
//!
//! The `"desugar"` pass runs between parse and eval in `Runtime::parse`.
//! Declarative `AstShapeDef::desugar` rules are lowered into
//! `TransformDef`s in `lang.transforms["desugar"]` at `Language::merge`
//! time (ADR-0094 §6, `handl-language`), and the `desugar.*` native
//! rewrites (`while`, `do-while`, `for`, `nullish`, `opt-chain`, `data`,
//! `match`) live in
//! `handl-plugin-engine-transform-rewrite::native::desugar` behind the
//! `NativeRewrite` registry — consulted as a fallback for kinds with no
//! declarative rule, preserving the legacy ordering.
//!
//! ## Pipeline ordering (ADR-0082, Phase 1.2)
//!
//! The desugar pass runs **at parse time** — `Runtime::parse` calls
//! `apply_desugar` immediately after the Pratt parser produces the raw AST,
//! before returning the AST to the caller. This means:
//!
//! - `Runtime::eval` always sees the desugared AST (no raw `data`/`while`/
//!   `for`/`nullish`/`opt-chain` nodes).
//! - `Runtime::typecheck` always sees the desugared AST (the typechecker
//!   and NbeTypecheckEngine never observe raw `data` nodes — they see the
//!   desugared `block` of predicate/constructor/recursor assignments).
//! - `Kit::typecheck` calls `Kit::parse` (which desugars) before
//!   `Runtime::typecheck`, so the full pipeline is: lex → parse → desugar
//!   → typecheck.
//!
//! The bootstrap `check_module` in `plugin-engine-typecheck` parses
//! directly (without desugar) due to a cyclic dependency constraint
//! (`plugin-engine-typecheck` cannot depend on `handl-runtime`). This is
//! a known limitation: the bootstrap checker does not handle `data`
//! declarations. The runtime pipeline (`Kit`/`Runtime`) is the
//! recommended path for typechecking `data` declarations.

use handl_language::{Ast, AstRef, LangError, Language};
use handl_plugin_engine_transform_rewrite::RewriteEngine;

/// Apply the `"desugar"` transform pass to an [`Ast`].
///
/// Delegates to [`RewriteEngine::run_pass`] with the desugar native
/// registry wired ([`RewriteEngine::with_desugar_natives`]). The root
/// node of the input AST is the last node in the arena (Pratt
/// convention); the output preserves this convention.
///
/// # Errors
///
/// Returns [`LangError`] when a transform rule fails (no silent
/// fallthrough — a failed transform never returns the un-transformed
/// AST, per ADR-0094 "Production readiness").
pub fn apply_desugar(lang: &Language, ast: &Ast) -> Result<Ast, LangError> {
    if ast.is_empty() {
        return Ok(ast.clone());
    }
    let engine = RewriteEngine::new().with_desugar_natives();
    let root = AstRef::new(ast.len() - 1);
    engine.run_pass(lang, ast, root, "desugar").map(|out| out.ast)
}
