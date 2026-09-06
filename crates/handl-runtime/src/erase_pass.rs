//! Proof-erasure pass (ADR-0032 item 6).
//!
//! After the typechecker succeeds, this pass walks the AST and replaces any
//! node whose inferred type is a proof constructor (`type.refl`,
//! `type.is_evidence`, or `type.empty`) with an `erased` node. The `erased`
//! node has no runtime cost: it evaluates to `Value::Null`.
//!
//! The pass is an **in-place arena mutation**: node kinds and payloads are
//! rewritten, but arena indices are preserved, so all parent references remain
//! valid.

use std::collections::HashMap;

use handl_language::{Ast, AstRef, Language};
use serde_json::Value;

/// Tags that identify singleton proof values in the `handl.canonical`
/// type registry. A `Value` carrying one of these `__tag__` fields is safe
/// to erase at runtime.
const PROOF_TAGS: &[&str] = &["type.refl", "type.is_evidence", "type.empty"];

/// Replace proof-typed AST nodes with `erased` nodes.
///
/// `node_types` maps each visited `AstRef::index()` to the `Value` type it was
/// checked/synthesized against. If a node's type is a tagged proof constructor
/// from `handl.canonical`, its kind is rewritten to `erased` and its payload
/// is replaced with `Value::Null`.
///
/// The `erased` node kind is resolved from `lang.ast` by identity (`erased`)
/// when present; otherwise it falls back to the canonical string `"erased"`.
pub fn apply_erase(lang: &Language, ast: &mut Ast, node_types: &HashMap<usize, Value>) {
    let erased_kind = lang
        .ast
        .get_by_identity("erased")
        .map(|def| def.node_kind.clone())
        .unwrap_or_else(|| "erased".to_string());

    for i in 0..ast.len() {
        let Some(ty) = node_types.get(&i) else {
            continue;
        };
        if !is_proof_value(ty) {
            continue;
        }
        if let Some(node) = ast.get_mut(AstRef::new(i)) {
            node.kind = erased_kind.clone();
            node.payload = Value::Null;
        }
    }
}

/// Returns `true` when `value` is a tagged proof constructor from the
/// `handl.canonical` registry.
fn is_proof_value(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    let Some(tag) = obj.get("__tag__").and_then(|v| v.as_str()) else {
        return false;
    };
    if !PROOF_TAGS.contains(&tag) {
        return false;
    }
    // If a registry is present, only treat `handl.canonical` values as proofs.
    if let Some(reg) = obj.get("__registry__").and_then(|v| v.as_str()) {
        reg == "handl.canonical"
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, Language};
    use serde_json::json;

    fn proof_value(tag: &str) -> Value {
        json!({"__tag__": tag, "__registry__": "handl.canonical"})
    }

    #[test]
    fn erases_proof_typed_node_in_place() {
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(Value::from(42)));

        let mut node_types = HashMap::new();
        node_types.insert(0, proof_value("type.refl"));

        apply_erase(&Language::new(), &mut ast, &node_types);

        let node = ast.get(AstRef::new(0)).unwrap();
        assert_eq!(node.kind, "erased");
        assert!(node.payload.is_null());
    }

    #[test]
    fn leaves_non_proof_nodes_unchanged() {
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(Value::from(42)));

        let mut node_types = HashMap::new();
        node_types.insert(
            0,
            json!({"__tag__": "type.number", "__registry__": "handl.canonical"}),
        );

        apply_erase(&Language::new(), &mut ast, &node_types);

        let node = ast.get(AstRef::new(0)).unwrap();
        assert_eq!(node.kind, "value");
        assert_eq!(node.payload, Value::from(42));
    }

    #[test]
    fn ignores_nodes_without_type_info() {
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(Value::from(42)));

        let node_types = HashMap::new();

        apply_erase(&Language::new(), &mut ast, &node_types);

        let node = ast.get(AstRef::new(0)).unwrap();
        assert_eq!(node.kind, "value");
        assert_eq!(node.payload, Value::from(42));
    }

    #[test]
    fn handles_missing_registry_gracefully() {
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(Value::Null));

        let mut node_types = HashMap::new();
        node_types.insert(0, json!({"__tag__": "type.refl"})); // no __registry__

        apply_erase(&Language::new(), &mut ast, &node_types);

        let node = ast.get(AstRef::new(0)).unwrap();
        assert_eq!(node.kind, "erased");
    }
}
