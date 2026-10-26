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
    // Normalize surface family-record declarations into the internal `data`
    // node the `desugar.data` rule consumes. The parser emits an `alt` of
    // `assign`s (`A(params) := c0 | c1`, optionally preceded by a stray
    // `var "data"`); both spellings lower to the same `data` shape here.
    let mut ast = ast.clone();
    lower_family_record_decls(&mut ast);
    let ast = &ast;
    let engine = RewriteEngine::new().with_desugar_natives();
    let root = AstRef::new(ast.len() - 1);
    let mut out = engine.run_pass(lang, ast, root, "desugar")?.ast;
    splice_decl_blocks(lang, &mut out);
    Ok(out)
}

/// Lower a surface family-record declaration into a `data` node.
///
/// The post-ADR-0158 surface is `A(a : Type) := c0 | c1` (the `data`
/// keyword is retired but still accepted as a stray leading token). The
/// parser represents that as an `alt` whose arms are `assign`s pairing the
/// type expression with each constructor:
///
/// ```text
/// alt[ assign(call(A, a:Type), var c0), assign(call(A, a:Type), c1(x:a)) ]
/// ```
///
/// `desugar.data` wants the internal shape `data { children: [type, ctors] }`
/// (`hnl_engine_transform_rewrite::native::desugar::parse_data_payload`).
/// This pass rewrites **in place** — the record's first `assign` node becomes
/// the `data` node and the `alt` node becomes the constructor list — so the
/// arena keeps its "root is last" convention (no nodes are pushed).
///
/// Only multi-constructor records are lowered: a single `x := y` binding is
/// indistinguishable from a one-constructor family at this level, and
/// mistaking one for the other would silently swallow a plain binding.
fn lower_family_record_decls(ast: &mut Ast) {
    for i in 0..ast.len() {
        let Some(children) = ast
            .get(AstRef::new(i))
            .and_then(|n| n.payload.get("children"))
            .and_then(Value::as_array)
            .cloned()
        else {
            continue;
        };
        let mut rewritten: Vec<Value> = Vec::with_capacity(children.len());
        let mut changed = false;
        let mut idx = 0;
        while idx < children.len() {
            // A retired `data` keyword parses as a bare `var "data"`
            // statement; drop it when it introduces a family record.
            let is_data_keyword = children[idx]
                .as_u64()
                .and_then(|c| ast.get(AstRef::new(c as usize)))
                .is_some_and(|n| n.kind == "var" && n.payload.as_str() == Some("data"));
            let record_at = if is_data_keyword { idx + 1 } else { idx };
            let record = children
                .get(record_at)
                .and_then(|v| v.as_u64())
                .and_then(|c| family_record(ast, c as usize));
            let Some((record_idx, type_idx, ctors)) = record else {
                rewritten.push(children[idx].clone());
                idx += 1;
                continue;
            };
            changed = true;
            let data_idx = ctors_data_slot(ast, record_idx);
            // The record's first `assign` node becomes the `data` node …
            if let Some(node) = ast.get_mut(AstRef::new(data_idx)) {
                node.kind = "data".to_string();
                node.payload = serde_json::json!({
                    "children": [Value::from(type_idx as u64), Value::from(record_idx as u64)],
                });
            }
            // … and the `alt` node becomes the constructor list.
            if let Some(node) = ast.get_mut(AstRef::new(record_idx)) {
                node.payload = serde_json::json!({
                    "children": ctors.iter().map(|c| Value::from(*c as u64)).collect::<Vec<_>>(),
                });
            }
            rewritten.push(Value::from(data_idx as u64));
            idx = record_at + 1;
        }
        if changed {
            if let Some(node) = ast.get_mut(AstRef::new(i)) {
                if let Some(obj) = node.payload.as_object_mut() {
                    obj.insert("children".to_string(), Value::Array(rewritten));
                }
            }
        }
    }
}

/// Recognize a family-record `alt` as the parser emits it: the **first** arm
/// is `assign(type_expr, c0)` (it pairs the declaration with its first
/// constructor) and every later arm is a constructor node. Returns the
/// `alt` index, the type-expression index, and the constructor indices.
fn family_record(ast: &Ast, alt_idx: usize) -> Option<(usize, usize, Vec<usize>)> {
    let alt = ast.get(AstRef::new(alt_idx))?;
    if alt.kind != "alt" {
        return None;
    }
    let arms = alt.payload.get("children")?.as_array()?;
    if arms.len() < 2 {
        return None;
    }
    let first = arms.first()?.as_u64()? as usize;
    let node = ast.get(AstRef::new(first))?;
    if node.kind != "assign" {
        return None;
    }
    let pair = node.payload.get("children")?.as_array()?;
    if pair.len() != 2 {
        return None;
    }
    let type_idx = pair[0].as_u64()? as usize;
    let mut ctors = vec![pair[1].as_u64()? as usize];
    for arm in arms.iter().skip(1) {
        let arm_idx = arm.as_u64()? as usize;
        // Later arms are bare constructors; a stray `assign` means this is
        // an ordinary disjunction of assignments, not a family record.
        if ast.get(AstRef::new(arm_idx))?.kind == "assign" {
            return None;
        }
        ctors.push(arm_idx);
    }
    Some((alt_idx, type_idx, ctors))
}

/// The arena slot that becomes the `data` node: the record's first `assign`
/// node (its `children` are replaced wholesale).
fn ctors_data_slot(ast: &Ast, alt_idx: usize) -> usize {
    ast.get(AstRef::new(alt_idx))
        .and_then(|n| n.payload.get("children"))
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|v| v.as_u64())
        .unwrap_or(alt_idx as u64) as usize
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
            let Some(children) = node.payload.get("children").and_then(Value::as_array) else {
                continue;
            };
            let mut changed = false;
            let mut spliced: Vec<Value> = Vec::with_capacity(children.len());
            for entry in children {
                let is_splice = entry
                    .as_u64()
                    .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                    .map(|child| {
                        child.payload.get("__splice__").and_then(Value::as_bool) == Some(true)
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
            if changed { Some(spliced) } else { None }
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
