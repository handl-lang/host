//! Exec-transform type-erasure pass (ADR-0154 §6, generalizing ADR-0151).
//!
//! ADR-0151 established that type annotations are checker-owned static
//! metadata the exec engine must never evaluate; ADR-0154 §6 generalizes
//! that into a uniform rule: **type-level positions erase at exec**. The
//! `"exec-erasure"` transform pass runs after typechecking (post
//! `runtime_assert_pass` + `erase_pass`) and before evaluation, rewriting
//! every node a rule declares erasable:
//!
//! - `type-annot` — `expr : T` rewrites to the `subject` child via
//!   `TransformRule::Replace` (the `annot` subtree becomes unreachable);
//!   shipped declaratively as `transform.exec-erasure.type-annot` in the
//!   canonical kit's `transforms.yaml`. This is the AST-level form of
//!   ADR-0151's `eval.type-annot` erasure — the annotation never reaches
//!   an eval scheme at all.
//! - `Pi`/`Sigma`/`Forall`/`Family` type expressions and former
//!   applications (`Hom`/`Arr`/`Type(i)` spellings, `{"__former__"}`
//!   values) living *inside* erased positions vanish with the position.
//!   The same content in a *runnable* position is not erased — it is
//!   live-but-stuck code, which [`crate::artifact_lint`] turns into a
//!   diagnostic rather than silently null-ing it (ADR-0154 §6).
//! - `data` declarations already desugar to former/ctor/rec bindings at
//!   parse; their type metadata surfaces as `type-annot` nodes inside the
//!   emitted bindings and erases with the same rule. A raw `data` node
//!   reaching exec is a pipeline bug — `eval.data` raises
//!   `desugar_error`.
//!
//! The pass is **data-driven**: languages declare their own erasable
//! node kinds as `pass: "exec-erasure"` `TransformDef`s in
//! `lang.transforms` — no node-kind dispatch lives here (ADR-0044,
//! engine-genericity). A language with no `exec-erasure` rules passes
//! through unchanged.
//!
//! ## Root-elimination and DCE
//!
//! `Replace` rules map the matched node onto an existing descendant, so a
//! root `type-annot` (e.g. a bare `42 : Nat` program) maps the *input*
//! root to a mid-arena output index and leaves the erased annotation
//! subtree behind as unreachable arena entries. [`Ast::subtree`]
//! re-lays the reachable arena post-order — dropping orphaned type-level
//! subtrees (the dead-code half of erasure, ADR-0154 §6) and restoring
//! the root-is-last Pratt convention the eval engines assume.

use handl_language::{Ast, AstRef, LangError, Language};
use hnl_engine_transform_rewrite::RewriteEngine;

/// The transform pass erased at exec (ADR-0154 §6): rules with
/// `pass: "exec-erasure"` in `lang.transforms` strip type-level
/// positions from the runnable artifact.
pub const EXEC_ERASURE_PASS: &str = "exec-erasure";

/// Whether `lang` declares any `exec-erasure` rules.
///
/// When no rules are declared the pass is a pass-through and
/// [`apply_exec_erasure`] returns the arena unchanged (fast path — most
/// test `Language`s carry no transform fragments).
pub fn has_exec_erasure_rules(lang: &Language) -> bool {
    lang.transforms.iter().any(|def| def.pass == EXEC_ERASURE_PASS)
}

/// Apply the `"exec-erasure"` transform pass to `ast`, returning the
/// runnable arena (ADR-0154 §6).
///
/// Delegates to [`RewriteEngine::run_pass`] — the rules themselves are
/// language data (`lang.transforms`), so languages extend erasure by
/// declaring rules, not by editing this pass. The output arena is then
/// compacted through [`Ast::subtree`] so unreachable erased subtrees are
/// dropped and the root is the last node (Pratt convention).
///
/// Only declarative rules (`!Declarative`/`!Replace`) are wired — a
/// `!Native` rule in this pass fails loudly (no silent fallthrough, per
/// ADR-0094); hosts needing natives run [`RewriteEngine`] themselves
/// with a wired registry.
///
/// # Errors
///
/// Returns [`LangError`] when a transform rule fails — e.g. a `Replace`
/// path that does not resolve to a descendant.
pub fn apply_exec_erasure(lang: &Language, ast: &Ast) -> Result<Ast, LangError> {
    if ast.is_empty() || !has_exec_erasure_rules(lang) {
        return Ok(ast.clone());
    }
    let engine = RewriteEngine::new();
    let root = AstRef::new(ast.len() - 1);
    let out = engine.run_pass(lang, ast, root, EXEC_ERASURE_PASS)?;
    out.ast.subtree(out.root)
}

/// Prepare the final runnable artifact (ADR-0154 §6).
///
/// Composes the two halves of erasure: the `"exec-erasure"` transform
/// pass strips type-level positions ([`apply_exec_erasure`]), then
/// [`artifact_lint::check_exec_artifact`](crate::artifact_lint) asserts
/// the invariant — only 1-arrows + runtime values remain. Anything the
/// pass could not consume (a stuck-but-referenced term is *live* code
/// DCE cannot remove) becomes a diagnostic, not silent unsafety.
///
/// Called by every [`crate::Runtime`] entry point that dispatches an
/// artifact to an exec/compile engine (`eval`, `eval_with_globals`,
/// `eval_shared_*`, `compile`), so "type-level positions erase at exec"
/// holds uniformly across exec engines.
///
/// # Errors
///
/// Returns [`LangError`] when a transform rule fails or the artifact
/// lint finds residual type-level content.
pub fn prepare_exec_artifact(lang: &Language, ast: &Ast) -> Result<Ast, LangError> {
    let ast = apply_exec_erasure(lang, ast)?;
    crate::artifact_lint::check_exec_artifact(lang, &ast)?;
    Ok(ast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, TransformDef, TransformRule};
    use serde_json::json;

    fn lang_with_erasure_rule() -> Language {
        let mut lang = Language::new();
        lang.transforms
            .merge(TransformDef {
                id: "transform.exec-erasure.type-annot".to_string(),
                pass: EXEC_ERASURE_PASS.to_string(),
                node_kind: "type-annot".to_string(),
                priority: 0,
                when: None,
                rule: TransformRule::Replace { path: vec![0] },
                replaces: vec![],
                requires: vec![],
            })
            .unwrap();
        lang
    }

    fn type_annot_ast() -> Ast {
        // [0] subject value 42, [1] annot `var Nat`, [2] type-annot[0,1].
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!(42)));
        ast.push(AstNode::new("var").with_payload(json!("Nat")));
        ast.push(
            AstNode::new("type-annot").with_payload(json!({"children": [0, 1]})),
        );
        ast
    }

    #[test]
    fn erases_type_annot_to_subject() {
        let lang = lang_with_erasure_rule();
        let ast = type_annot_ast();
        let out = apply_exec_erasure(&lang, &ast).unwrap();
        // The annotation subtree is dropped; the subject is the root.
        assert_eq!(out.len(), 1);
        let root = out.get(AstRef::new(0)).unwrap();
        assert_eq!(root.kind, "value");
        assert_eq!(root.payload, json!(42));
    }

    #[test]
    fn erases_nested_type_annot_inside_parent() {
        // [0] 1, [1] `var Int`, [2] type-annot[0,1], [3] 2, [4] add[2,3]
        let lang = lang_with_erasure_rule();
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!(1)));
        ast.push(AstNode::new("var").with_payload(json!("Int")));
        ast.push(AstNode::new("type-annot").with_payload(json!({"children": [0, 1]})));
        ast.push(AstNode::new("value").with_payload(json!(2)));
        ast.push(AstNode::new("binary.add").with_payload(json!({"children": [2, 3]})));
        let out = apply_exec_erasure(&lang, &ast).unwrap();
        let root = out.get(AstRef::new(out.len() - 1)).unwrap();
        assert_eq!(root.kind, "binary.add");
        // No `type-annot`/`var Int` nodes remain reachable.
        assert!(out.nodes().iter().all(|n| n.kind != "type-annot"));
        assert_eq!(out.len(), 3); // value(1), value(2), add
    }

    #[test]
    fn no_rules_is_pass_through() {
        let lang = Language::new();
        let ast = type_annot_ast();
        let out = apply_exec_erasure(&lang, &ast).unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out.nodes()[2].kind, "type-annot");
    }

    #[test]
    fn empty_ast_passes_through() {
        let lang = lang_with_erasure_rule();
        let out = apply_exec_erasure(&lang, &Ast::new()).unwrap();
        assert!(out.is_empty());
    }
}
