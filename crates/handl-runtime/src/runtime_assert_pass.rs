//! Runtime-assert lowering pass (ADR-0068 §1 "graduated guarantees").
//!
//! When the typechecker cannot normalize a predicate annotation
//! (`Verdict::Blocked`), it accepts the annotation statically but records a
//! `runtime_assert` diagnostic. This pass rewrites the AST so that each
//! `type-annot` node whose annotation is a predicate is replaced by a
//! `runtime-assert` node that evaluates the predicate at runtime and raises
//! a `LangError` on failure.
//!
//! The pass is an **in-place arena mutation**: each `type-annot` node has its
//! `kind` changed to `runtime-assert` and its children reordered from
//! `[subject, annot]` to `[predicate, expr]` = `[annot, subject]`. Arena
//! indices are preserved, so all parent references remain valid.
//!
//! ## Over-eager wrapping
//!
//! The pass wraps **all** `type-annot` nodes, not just the blocked ones,
//! because the typecheck diagnostics do not currently carry enough info to
//! locate the exact annotation site. This is safe: the `runtime_assert`
//! native op passes through the value unchanged when the predicate is not a
//! closure (e.g. a plain `type.number` tag), so non-predicate annotations
//! incur only one extra eval call with no behavioral change. The pass is
//! only run when at least one `runtime_assert` diagnostic exists, so
//! programs without blocked predicates pay no overhead.

use handl_language::{Ast, AstRef, Language};
use serde_json::Value;

/// Rewrite all `type-annot` nodes in `ast` into `runtime-assert` nodes.
///
/// For each `type-annot` node with children `[subject, annot]`, the node's
/// `kind` is changed to `runtime-assert` and the children are reordered to
/// `[annot, subject]` (predicate = annot, expr = subject), matching the
/// `runtime-assert` shape declared in `ast.yaml`.
///
/// Child roles are resolved via `lang.child_role_index` so the pass is
/// data-driven (ADR-0081). If the `type-annot` shape declares no roles, the
/// pass falls back to positional `[0 = subject, 1 = annot]`.
///
/// The mutation is in-place: arena indices are preserved.
pub fn apply_runtime_asserts(lang: &Language, ast: &mut Ast) {
    // Resolve type-annot child roles once (data-driven, ADR-0081).
    let subject_idx = lang
        .child_role_index("type-annot", "subject")
        .ok()
        .flatten()
        .unwrap_or(0);
    let annot_idx = lang
        .child_role_index("type-annot", "annot")
        .ok()
        .flatten()
        .unwrap_or(1);

    // Resolve runtime-assert child roles once.
    let pred_idx = lang
        .child_role_index("runtime-assert", "predicate")
        .ok()
        .flatten()
        .unwrap_or(0);
    let expr_idx = lang
        .child_role_index("runtime-assert", "expr")
        .ok()
        .flatten()
        .unwrap_or(1);

    for i in 0..ast.len() {
        let node_ref = AstRef::new(i);
        let is_type_annot = ast
            .get(node_ref)
            .map(|n| n.kind == "type-annot")
            .unwrap_or(false);
        if !is_type_annot {
            continue;
        }

        // Extract the subject and annot child arena indices from the
        // type-annot node's payload.
        let (subject_arena, annot_arena) = {
            let node = match ast.get(node_ref) {
                Some(n) => n,
                None => continue,
            };
            let children = match node.payload.get("children").and_then(|c| c.as_array()) {
                Some(arr) => arr,
                None => continue,
            };
            let s = match children.get(subject_idx).and_then(|v| v.as_u64()) {
                Some(v) => v as usize,
                None => continue,
            };
            let a = match children.get(annot_idx).and_then(|v| v.as_u64()) {
                Some(v) => v as usize,
                None => continue,
            };
            (s, a)
        };

        // Reorder children: [annot, subject] → predicate = annot, expr = subject.
        // Build the new children array indexed by runtime-assert roles.
        let new_children_len = pred_idx.max(expr_idx) + 1;
        let mut new_children = vec![Value::Null; new_children_len];
        new_children[pred_idx] = Value::from(annot_arena as u64);
        new_children[expr_idx] = Value::from(subject_arena as u64);

        // Mutate the node in-place: change kind + reorder children.
        if let Some(node) = ast.get_mut(node_ref) {
            node.kind = "runtime-assert".to_string();
            if let Some(obj) = node.payload.as_object_mut() {
                obj.insert("children".to_string(), Value::Array(new_children));
            } else {
                // payload is not an object — replace it entirely.
                node.payload = Value::Object(serde_json::Map::from_iter(std::iter::once((
                    "children".to_string(),
                    Value::Array(new_children),
                ))));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, Language};

    fn make_lang() -> Language {
        // Build a minimal language with the runtime-assert and type-annot
        // shapes. In practice the canonical kit data is loaded, but for
        // unit tests we construct a language that has the shapes.
        let lang = Language::new();
        // The shapes are loaded from YAML in the canonical kit; for testing
        // we rely on the canonical data being embedded. If the language is
        // empty, role_index falls back to None and we use positional.
        lang
    }

    #[test]
    fn rewrites_type_annot_to_runtime_assert_in_place() {
        let mut ast = Ast::new();
        // subject node (index 0): payload = 42
        let subject = ast.push(AstNode::new("literal").with_payload(Value::from(42)));
        // annot node (index 1): a function node (predicate)
        ast.push(AstNode::new("function").with_payload(Value::Null));
        // type-annot node (index 2): children = [subject, annot]
        let mut payload = serde_json::Map::new();
        payload.insert(
            "children".to_string(),
            Value::Array(vec![
                Value::from(subject.0 as u64),
                Value::from(1u64), // annot at index 1
            ]),
        );
        ast.push(AstNode::new("type-annot").with_payload(Value::Object(payload)));

        let lang = make_lang();
        apply_runtime_asserts(&lang, &mut ast);

        // The node at index 2 should now be runtime-assert.
        let node = ast.get(AstRef::new(2)).unwrap();
        assert_eq!(node.kind, "runtime-assert");
        // Children should be [annot(1), subject(0)] = [predicate, expr].
        let children = node.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children[0], Value::from(1u64)); // predicate = annot
        assert_eq!(children[1], Value::from(0u64)); // expr = subject
    }
}
