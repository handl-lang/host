//! Data-driven AST desugar pass (ADR-0053 §7).
//!
//! Runs between parse and eval in `Runtime::parse`. For each node whose
//! `AstShapeDef` has `desugar: Some(rule)`, the pass rewrites the node into
//! `rule.target_kind` with children specified by `rule.children`. The
//! rewrite is recursive: a `DesugarChild::Create` node may itself contain
//! `DesugarChild` children, and the resulting AST is walked again so that
//! newly-created nodes can themselves be desugared if their kinds have
//! rules.
//!
//! The pass is a pure `Ast → Ast` transform. It allocates new nodes for
//! `Create` children and reuses original arena indices for `Keep` children.
//! The root node of the input AST is the last node in the arena (Pratt
//! convention); the output preserves this convention.

use handl_language::{
    Ast, AstDesugarChild, AstDesugarRule, AstNode, AstRef, AstShapeDef, Language,
};
use serde_json::{Map, Value};

/// Apply all data-driven desugar rules to an [`Ast`].
///
/// Walks the AST bottom-up, rewriting nodes whose kind has a `desugar`
/// rule. Returns a new [`Ast`] with the rewritten nodes.
pub fn apply_desugar(lang: &Language, ast: &Ast) -> Ast {
    let mut out = Ast::new();
    // Map old arena indices → new arena indices.
    let mut remap: Vec<Option<usize>> = vec![None; ast.len()];
    // Walk in insertion order (children before parents — Pratt convention).
    for i in 0..ast.len() {
        let old_ref = AstRef::new(i);
        let node = match ast.get(old_ref) {
            Some(n) => n,
            None => continue,
        };
        // First, remap this node's children indices to the new arena.
        let remapped = remap_children(node, &remap);
        // Check for a desugar rule on this node kind.
        let shape = lang.ast.get_by_identity(&node.kind);
        if let Some(shape) = shape {
            if let Some(rule) = &shape.desugar {
                // Build the replacement node from the rule. Pass the
                // ORIGINAL (pre-remap) node so `Keep(i)` can look up old
                // arena indices and translate them via `remap`.
                let new_ref = build_desugar(&rule, node, &mut out, &remap);
                remap[i] = Some(new_ref.index());
                continue;
            }
        }
        // No desugar: push the remapped node.
        let new_ref = out.push(remapped);
        remap[i] = Some(new_ref.index());
    }
    out
}

/// Remap the `children` array in a node's payload from old to new indices.
fn remap_children(node: &AstNode, remap: &[Option<usize>]) -> AstNode {
    let mut new_payload = node.payload.clone();
    if let Some(children) = new_payload.get_mut("children").and_then(|c| c.as_array_mut()) {
        for child in children.iter_mut() {
            if let Some(old_idx) = child.as_u64() {
                if let Some(Some(new_idx)) = remap.get(old_idx as usize) {
                    *child = Value::from(*new_idx);
                }
            }
        }
    }
    AstNode {
        kind: node.kind.clone(),
        span: node.span,
        payload: new_payload,
    }
}

/// Build a desugar replacement node from an [`AstDesugarRule`].
///
/// The rule specifies `target_kind` and a list of [`AstDesugarChild`]s.
/// Each child is either `Keep(i)` (use the remapped arena index of the
/// original node's child `i`) or `Create { kind, payload, children }`
/// (allocate a new node).
///
/// `original` is the original (pre-remap) node — its `children` array
/// holds OLD arena indices. The `remap` table translates old → new.
fn build_desugar(
    rule: &AstDesugarRule,
    original: &AstNode,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> AstRef {
    let children: Vec<Value> = rule
        .children
        .iter()
        .map(|spec| build_child(spec, original, out, remap))
        .collect();
    let mut payload = Map::new();
    if !children.is_empty() {
        payload.insert("children".to_string(), Value::Array(children));
    }
    let node = AstNode {
        kind: rule.target_kind.clone(),
        span: original.span,
        payload: Value::Object(payload),
    };
    out.push(node)
}

/// Build a single child of a desugar replacement node.
fn build_child(
    spec: &AstDesugarChild,
    original: &AstNode,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Value {
    match spec {
        AstDesugarChild::Keep(i) => {
            // Look up original node's child i and remap it.
            let old_idx = original
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.get(*i))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            match old_idx.and_then(|idx| remap.get(idx).copied().flatten()) {
                Some(new_idx) => Value::from(new_idx),
                None => Value::Null,
            }
        }
        AstDesugarChild::Create {
            kind,
            payload,
            children,
        } => {
            let child_indices: Vec<Value> = children
                .iter()
                .map(|c| build_child(c, original, out, remap))
                .collect();
            let node_payload = if child_indices.is_empty() {
                payload.clone().unwrap_or(Value::Null)
            } else {
                let mut p = Map::new();
                p.insert("children".to_string(), Value::Array(child_indices));
                if let Some(extra) = payload {
                    if let Some(obj) = extra.as_object() {
                        for (k, v) in obj {
                            p.insert(k.clone(), v.clone());
                        }
                    }
                }
                Value::Object(p)
            };
            let node = AstNode {
                kind: kind.clone(),
                span: None,
                payload: node_payload,
            };
            let new_ref = out.push(node);
            Value::from(new_ref.index())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstShapeDef, FragmentSet};
    use serde_json::json;

    #[test]
    fn no_desugar_is_identity() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.lit", "lit"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        let out = apply_desugar(&lang, &ast);
        assert_eq!(out.len(), 1);
        assert_eq!(out.nodes()[0].kind, "lit");
    }

    #[test]
    fn if_to_match_desugar() {
        let mut lang = Language::new();
        // `if` shape with desugar → match
        let if_shape = AstShapeDef::new("ast.if", "if").with_desugar(AstDesugarRule {
            target_kind: "match".to_string(),
            children: vec![
                // Keep child 0 (cond) as scrutinee
                AstDesugarChild::Keep(0),
                // Create block { children: [arm1, arm2] }
                AstDesugarChild::Create {
                    kind: "block".to_string(),
                    payload: None,
                    children: vec![
                        // arm { children: [lit(true), then] }
                        AstDesugarChild::Create {
                            kind: "arm".to_string(),
                            payload: None,
                            children: vec![
                                AstDesugarChild::Create {
                                    kind: "lit".to_string(),
                                    payload: Some(json!(true)),
                                    children: vec![],
                                },
                                AstDesugarChild::Keep(1),
                            ],
                        },
                        // arm { children: [var("_"), else] }
                        AstDesugarChild::Create {
                            kind: "arm".to_string(),
                            payload: None,
                            children: vec![
                                AstDesugarChild::Create {
                                    kind: "var".to_string(),
                                    payload: Some(json!("_")),
                                    children: vec![],
                                },
                                AstDesugarChild::Keep(2),
                            ],
                        },
                    ],
                },
            ],
        });
        lang.ast.merge(if_shape).unwrap();
        // Other shapes (no desugar)
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.match", "match")).unwrap();

        // Build: if { children: [cond=0, then=1, else=2] }
        // where 0=lit(true), 1=lit(1), 2=lit(0)
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(true))); // 0
        ast.push(AstNode::new("lit").with_payload(json!(1))); // 1
        ast.push(AstNode::new("lit").with_payload(json!(0))); // 2
        ast.push(
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})), // 3
        );

        let out = apply_desugar(&lang, &ast);
        // The last node should be `match`, not `if`.
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "match");
        // match has children [cond, block]
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 2);
        // First child is the remapped cond (lit(true))
        let cond = out.get(AstRef::new(children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(cond.kind, "lit");
        assert_eq!(cond.payload, json!(true));
        // Second child is the block
        let block = out.get(AstRef::new(children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(block.kind, "block");
        let block_children = block.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(block_children.len(), 2);
        // Each arm
        let arm1 = out.get(AstRef::new(block_children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(arm1.kind, "arm");
        let arm2 = out.get(AstRef::new(block_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(arm2.kind, "arm");
    }

    #[test]
    fn nested_if_desugar() {
        let mut lang = Language::new();
        let if_shape = AstShapeDef::new("ast.if", "if").with_desugar(AstDesugarRule {
            target_kind: "match".to_string(),
            children: vec![
                AstDesugarChild::Keep(0),
                AstDesugarChild::Create {
                    kind: "block".to_string(),
                    payload: None,
                    children: vec![
                        AstDesugarChild::Create {
                            kind: "arm".to_string(),
                            payload: None,
                            children: vec![
                                AstDesugarChild::Create {
                                    kind: "lit".to_string(),
                                    payload: Some(json!(true)),
                                    children: vec![],
                                },
                                AstDesugarChild::Keep(1),
                            ],
                        },
                        AstDesugarChild::Create {
                            kind: "arm".to_string(),
                            payload: None,
                            children: vec![
                                AstDesugarChild::Create {
                                    kind: "var".to_string(),
                                    payload: Some(json!("_")),
                                    children: vec![],
                                },
                                AstDesugarChild::Keep(2),
                            ],
                        },
                    ],
                },
            ],
        });
        lang.ast.merge(if_shape).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.match", "match")).unwrap();

        // Build: outer if { children: [0, 1, 5] }
        //   0: lit(true)   (cond)
        //   1: lit(1)      (then)
        //   2: lit(true)   (inner cond)
        //   3: lit(2)      (inner then)
        //   4: lit(3)      (inner else)
        //   5: if { children: [2, 3, 4] }  (inner if = else of outer)
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(true))); // 0
        ast.push(AstNode::new("lit").with_payload(json!(1))); // 1
        ast.push(AstNode::new("lit").with_payload(json!(true))); // 2
        ast.push(AstNode::new("lit").with_payload(json!(2))); // 3
        ast.push(AstNode::new("lit").with_payload(json!(3))); // 4
        ast.push(AstNode::new("if").with_payload(json!({"children": [2, 3, 4]}))); // 5
        ast.push(AstNode::new("if").with_payload(json!({"children": [0, 1, 5]}))); // 6

        let out = apply_desugar(&lang, &ast);
        // The outer if (last node) should be desugared to match.
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "match");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 2);
        // The second arm's body (Keep(2) → original child 5 = inner if)
        // should be a match node (desugared), not null.
        let block = out.get(AstRef::new(children[1].as_u64().unwrap() as usize)).unwrap();
        let block_children = block.payload.get("children").unwrap().as_array().unwrap();
        let arm2 = out.get(AstRef::new(block_children[1].as_u64().unwrap() as usize)).unwrap();
        let arm2_children = arm2.payload.get("children").unwrap().as_array().unwrap();
        let else_body = out.get(AstRef::new(arm2_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(else_body.kind, "match", "nested if should be desugared to match");
    }
}
