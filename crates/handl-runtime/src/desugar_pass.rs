//! Data-driven AST desugar pass — thin delegate over the generic
//! transform engine (ADR-0094 §6).
//!
//! The `"desugar"` pass runs between parse and eval in `Runtime::parse`.
//! Declarative `AstShapeDef::desugar` rules are lowered into
//! `TransformDef`s in `lang.transforms["desugar"]` at `Language::merge`
//! time (ADR-0094 §6, `handl-language`), and the `desugar.*` native
//! rewrites (`while`, `do-while`, `for`, `nullish`, `opt-chain`, `data`,
//! `match`) live in
//! `hnl-engine-transform-rewrite::native::desugar` behind the
//! `NativeRewrite` registry — consulted as a fallback for kinds with no
//! declarative rule, preserving the legacy ordering.
//!
//! ## Pipeline ordering (ADR-0082, Phase 1.2)
//!
//! The desugar pass runs **at parse time for the exec pipeline** —
//! `Runtime::parse` is `Runtime::parse_raw` + `apply_desugar`. This means:
//!
//! - `Runtime::eval` always sees the desugared AST (no raw `data`/`while`/
//!   `for`/`nullish`/`opt-chain` nodes).
//! - The *typecheck* pipeline consumes the **raw** AST instead:
//!   `Runtime::parse_raw` / `Kit::parse_raw` feed `Kit::typecheck*`, and
//!   `Runtime::run_checked` checks the raw AST then desugars for eval.
//!   The NBE checker's type rules and judgments are authored against
//!   source-level shapes (`data` → `synth_data`, `match` →
//!   `synth_match_arms`); desugared output mints internal nodes (`lit`
//!   `__sym__` payloads, `__former__`/`__ctor__` objects) that have no
//!   typing rules by design — so `lex → parse → desugar → typecheck`
//!   was an integration bug, not the intended pipeline.
//!
//! The bootstrap `check_module` in `hnl-engine-typecheck` parses
//! directly (without desugar) due to a cyclic dependency constraint
//! (`hnl-engine-typecheck` cannot depend on `handl-runtime`). This is
//! a known limitation: the bootstrap checker does not handle `data`
//! declarations. The runtime pipeline (`Kit`/`Runtime`) is the
//! recommended path for typechecking `data` declarations.

use handl_language::{Ast, AstRef, LangError, Language};
use hnl_engine_transform_rewrite::RewriteEngine;
use serde_json::Value;

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
    let mut out = engine.run_pass(lang, ast, root, "desugar")?.ast;
    splice_decl_blocks(lang, &mut out);
    Ok(out)
}

/// Flatten desugar-minted *declaration blocks* into their enclosing
/// statement lists.
///
/// `desugar.data` rewrites a `data` node into a `block` of former /
/// constructor / recursor `assign`s carrying a `"__splice__": true`
/// payload flag (ADR-0152). In statement position that nested block
/// would evaluate in its own scope — the constructors would pop with
/// the scope and `Succ(Zero)` in a following statement would be
/// unbound. The emitted block is a *declaration group*, not a scope:
/// this pass replaces each flagged child with its own children so the
/// assigns bind in the enclosing block.
///
/// Nodes are processed in arena order (children before parents — the
/// desugar natives push children before the parents that index them),
/// so a flagged block nested inside another flagged block flattens
/// fully in a single bottom-up pass. Flagged nodes that remain
/// unreachable after splicing are harmless arena residue.
fn splice_decl_blocks(lang: &Language, ast: &mut Ast) {
    for i in 0..ast.len() {
        let new_children = {
            let Some(node) = ast.get(AstRef::new(i)) else {
                continue;
            };
            let Some(children) = node.payload.get("children").and_then(Value::as_array)
            else {
                continue;
            };
            let mut changed = false;
            let mut spliced: Vec<Value> = Vec::with_capacity(children.len());
            for entry in children {
                let is_splice = entry
                    .as_u64()
                    .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                    .map(|child| {
                        child.payload.get("__splice__").and_then(Value::as_bool)
                            == Some(true)
                            && lang
                                .child_role_index(&child.kind, "statements")
                                .ok()
                                .flatten()
                                .is_some()
                    })
                    .unwrap_or(false);
                if is_splice {
                    changed = true;
                    let sub = entry
                        .as_u64()
                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                        .and_then(|child| child.payload.get("children"))
                        .and_then(Value::as_array);
                    if let Some(sub) = sub {
                        spliced.extend(sub.iter().cloned());
                    }
                    continue;
                }
                spliced.push(entry.clone());
            }
            if changed {
                Some(spliced)
            } else {
                None
            }
        };
        if let Some(children) = new_children {
            if let Some(node) = ast.get_mut(AstRef::new(i)) {
                if let Some(obj) = node.payload.as_object_mut() {
                    obj.insert("children".to_string(), Value::Array(children));
                }
            }
        }
    }
}
