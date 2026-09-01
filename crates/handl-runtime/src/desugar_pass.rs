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
    Ast, AstDesugarChild, AstDesugarRule, AstNode, AstRef, Language,
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
                // ADR-0058: if the rule has a `when` condition, only fire
                // it when the original node's child at `child_index` has
                // kind `child_kind`. Otherwise fall through to the
                // no-desugar path (push the remapped node unchanged).
                if let Some(cond) = &rule.when {
                    if !matches_condition(ast, node, cond) {
                        let new_ref = out.push(remapped);
                        remap[i] = Some(new_ref.index());
                        continue;
                    }
                }
                // Build the replacement node from the rule. Pass the
                // ORIGINAL (pre-remap) node so `Keep(i)` can look up old
                // arena indices and translate them via `remap`.
                let new_ref = build_desugar(&rule, node, ast, &mut out, &remap);
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

/// Check whether a node's child at `cond.child_index` has kind
/// `cond.child_kind` (ADR-0058 `when` condition).
fn matches_condition(ast: &Ast, node: &AstNode, cond: &handl_language::AstDesugarCondition) -> bool {
    let child_idx = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(cond.child_index))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    match child_idx {
        Some(idx) => match ast.get(AstRef::new(idx)) {
            Some(child) => child.kind == cond.child_kind,
            None => false,
        },
        None => false,
    }
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
/// original node's child `i`), `KeepPath([i, j, ...])` (navigate through
/// descendant children by index path), or `Create { kind, payload, children }`
/// (allocate a new node).
///
/// `original` is the original (pre-remap) node — its `children` array
/// holds OLD arena indices. The `remap` table translates old → new.
/// `ast` is the full original AST, needed by `KeepPath` to look up
/// intermediate nodes during navigation.
fn build_desugar(
    rule: &AstDesugarRule,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> AstRef {
    let children: Vec<Value> = rule
        .children
        .iter()
        .flat_map(|spec| build_child(spec, original, ast, out, remap))
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

/// Build children of a desugar replacement node from a single spec.
///
/// Returns a `Vec<Value>` because `KeepRest` expands to multiple children.
/// All other variants produce exactly one child.
fn build_child(
    spec: &AstDesugarChild,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Vec<Value> {
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
                Some(new_idx) => vec![Value::from(new_idx)],
                None => vec![Value::Null],
            }
        }
        AstDesugarChild::KeepPath(path) => {
            // Navigate through the original AST's children by index path.
            // Start at the original node, follow each index into its
            // child's children array, and remap the final arena index.
            let mut current_node = original;
            let mut current_idx: Option<usize> = None;
            for &step in path {
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array());
                let next_idx = match children.and_then(|arr| arr.get(step)) {
                    Some(v) => v.as_u64().map(|v| v as usize),
                    None => None,
                };
                match next_idx {
                    Some(idx) => {
                        current_idx = Some(idx);
                        match ast.get(AstRef::new(idx)) {
                            Some(n) => current_node = n,
                            None => return vec![Value::Null],
                        }
                    }
                    None => return vec![Value::Null],
                }
            }
            match current_idx.and_then(|idx| remap.get(idx).copied().flatten()) {
                Some(new_idx) => vec![Value::from(new_idx)],
                None => vec![Value::Null],
            }
        }
        AstDesugarChild::Create {
            kind,
            payload,
            children,
        } => {
            let child_indices: Vec<Value> = children
                .iter()
                .flat_map(|c| build_child(c, original, ast, out, remap))
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
            vec![Value::from(new_ref.index())]
        }
        AstDesugarChild::KeepRest(start) => {
            // Keep all children from index `start` onward, each remapped
            // into the output arena (ADR-0060).
            let children = original
                .payload
                .get("children")
                .and_then(|c| c.as_array());
            match children {
                Some(arr) => arr
                    .iter()
                    .skip(*start)
                    .filter_map(|v| v.as_u64().map(|i| i as usize))
                    .filter_map(|idx| remap.get(idx).copied().flatten())
                    .map(|new_idx| Value::from(new_idx))
                    .collect(),
                None => vec![],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::AstShapeDef;
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
            when: None,
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
            when: None,
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

    #[test]
    fn entry_bracket_key_desugar() {
        // ADR-0058: `entry { children: [array { children: [expr] }, value] }`
        // desugars to `entry { children: [paren { children: [expr] }, value] }`
        // — the `array` wrapper is replaced by a `paren` wrapper so the
        // engine evaluates the inner expression as a computed key.
        //
        // Non-array keys (var, string, paren, etc.) pass through unchanged
        // because the `when` condition only fires for `array` children.
        let mut lang = Language::new();
        let entry_shape = AstShapeDef::new("ast.entry", "entry").with_desugar(AstDesugarRule {
            target_kind: "entry".to_string(),
            children: vec![
                // Create paren { children: [inner expr] }
                AstDesugarChild::Create {
                    kind: "paren".to_string(),
                    payload: None,
                    children: vec![AstDesugarChild::KeepPath(vec![0, 0])],
                },
                // Keep child 1 (value)
                AstDesugarChild::Keep(1),
            ],
            when: Some(handl_language::AstDesugarCondition {
                child_index: 0,
                child_kind: "array".to_string(),
            }),
        });
        lang.ast.merge(entry_shape).unwrap();
        // Other shapes (no desugar, just so the language is well-formed)
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.array", "array")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.paren", "paren")).unwrap();

        // Build: entry { children: [array{[0]}, value(1)] }
        //   0: var("k")        (computed key expr)
        //   1: array{[0]}      (the [k] bracket)
        //   2: value(1)        (the value)
        //   3: entry{[1, 2]}   (the entry to desugar)
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("k"))); // 0
        ast.push(AstNode::new("array").with_payload(json!({"children": [0]}))); // 1
        ast.push(AstNode::new("value").with_payload(json!(1))); // 2
        ast.push(AstNode::new("entry").with_payload(json!({"children": [1, 2]}))); // 3

        let out = apply_desugar(&lang, &ast);
        // The last node should still be `entry` (target_kind = entry).
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "entry");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 2);
        // Child 0 should be a `paren` node (not `array`).
        let key_node = out.get(AstRef::new(children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(key_node.kind, "paren");
        // The paren's child 0 should be the remapped var("k").
        let paren_children = key_node.payload.get("children").unwrap().as_array().unwrap();
        let inner = out.get(AstRef::new(paren_children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(inner.kind, "var");
        assert_eq!(inner.payload, json!("k"));
        // Child 1 should be the remapped value(1).
        let val = out.get(AstRef::new(children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(val.kind, "value");
        assert_eq!(val.payload, json!(1));
    }

    #[test]
    fn entry_non_array_key_passes_through() {
        // ADR-0058: entries whose key is NOT an `array` (e.g. a bare `var`)
        // pass through the desugar pass unchanged because the `when`
        // condition does not match.
        let mut lang = Language::new();
        let entry_shape = AstShapeDef::new("ast.entry", "entry").with_desugar(AstDesugarRule {
            target_kind: "entry".to_string(),
            children: vec![
                AstDesugarChild::Create {
                    kind: "paren".to_string(),
                    payload: None,
                    children: vec![AstDesugarChild::KeepPath(vec![0, 0])],
                },
                AstDesugarChild::Keep(1),
            ],
            when: Some(handl_language::AstDesugarCondition {
                child_index: 0,
                child_kind: "array".to_string(),
            }),
        });
        lang.ast.merge(entry_shape).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();

        // Build: entry { children: [var("n"), value(1)] }
        //   0: var("n")   (literal bare-id key)
        //   1: value(1)
        //   2: entry{[0, 1]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
        ast.push(AstNode::new("value").with_payload(json!(1))); // 1
        ast.push(AstNode::new("entry").with_payload(json!({"children": [0, 1]}))); // 2

        let out = apply_desugar(&lang, &ast);
        // The entry should pass through unchanged — key is still `var`, not `paren`.
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "entry");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        let key = out.get(AstRef::new(children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(key.kind, "var", "non-array key should pass through unchanged");
        assert_eq!(key.payload, json!("n"));
    }

    #[test]
    fn keep_rest_preserves_variable_length_tail() {
        // ADR-0060: KeepRest(N) expands to all children from index N
        // onward, each remapped into the output arena.
        let mut lang = Language::new();
        let shape = AstShapeDef::new("ast.call", "call").with_desugar(AstDesugarRule {
            target_kind: "json".to_string(),
            children: vec![
                AstDesugarChild::Keep(0),
                AstDesugarChild::KeepRest(1),
            ],
            when: None,
        });
        lang.ast.merge(shape).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.json", "json")).unwrap();

        // Build: call { children: [0, 1, 2] }
        //   0: json("{\"a\": $0}")  (template)
        //   1: lit(42)             (arg1)
        //   2: lit("hello")        (arg2)
        //   3: call{[0,1,2]}       (the call to desugar)
        let mut ast = Ast::new();
        ast.push(AstNode::new("json").with_payload(json!("{\"a\": $0}"))); // 0
        ast.push(AstNode::new("lit").with_payload(json!(42))); // 1
        ast.push(AstNode::new("lit").with_payload(json!("hello"))); // 2
        ast.push(AstNode::new("call").with_payload(json!({"children": [0, 1, 2]}))); // 3

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "json");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // Should have 3 children: [template, arg1, arg2]
        assert_eq!(children.len(), 3);
        // Child 0 is the json template
        let template = out.get(AstRef::new(children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(template.kind, "json");
        // Children 1 and 2 are the args
        let arg1 = out.get(AstRef::new(children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(arg1.kind, "lit");
        assert_eq!(arg1.payload, json!(42));
        let arg2 = out.get(AstRef::new(children[2].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(arg2.kind, "lit");
        assert_eq!(arg2.payload, json!("hello"));
    }

    #[test]
    fn keep_rest_zero_tail() {
        // ADR-0060: KeepRest(N) with no children from N onward produces
        // zero children in the output.
        let mut lang = Language::new();
        let shape = AstShapeDef::new("ast.call", "call").with_desugar(AstDesugarRule {
            target_kind: "json".to_string(),
            children: vec![
                AstDesugarChild::Keep(0),
                AstDesugarChild::KeepRest(1),
            ],
            when: None,
        });
        lang.ast.merge(shape).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.json", "json")).unwrap();

        let mut ast = Ast::new();
        ast.push(AstNode::new("json").with_payload(json!("{}"))); // 0
        ast.push(AstNode::new("call").with_payload(json!({"children": [0]}))); // 1

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "json");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // Only child 0 (the template); KeepRest(1) produces nothing
        assert_eq!(children.len(), 1);
    }
}
