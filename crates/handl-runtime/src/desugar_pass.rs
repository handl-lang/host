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
        // No declarative desugar rule: try a Rust-level desugar for
        // ADR-0074 v0.1 sugar forms (while, do-while, for, nullish,
        // opt-chain, if). These forms require fresh-name generation,
        // pattern construction, or recursive structure building that the
        // declarative `DesugarRule` format cannot express.
        if let Some(new_ref) = try_rust_desugar(node, ast, &mut out, &remap) {
            remap[i] = Some(new_ref.index());
            continue;
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

// ---------------------------------------------------------------------------
// Rust-level desugar passes for ADR-0074 v0.1 sugar forms (ADR-0074 v0.1).
//
// The declarative `DesugarRule` format in ast.yaml cannot express these
// rewrites because they require fresh-name generation, pattern construction,
// or recursive structure building. Each function below takes the original
// (pre-remap) node, the full original AST, the output arena, and the
// old→new remap table, and returns the replacement node's `AstRef` in the
// output arena.
// ---------------------------------------------------------------------------

/// Get the remapped new arena index of `original`'s child `i`.
///
/// Returns `None` if the child doesn't exist or wasn't remapped.
fn remap_child(original: &AstNode, remap: &[Option<usize>], i: usize) -> Option<usize> {
    let old_idx = original
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(i))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    old_idx.and_then(|idx| remap.get(idx).copied().flatten())
}

/// Get all remapped child indices from `start` onward.
fn remap_children_from(
    original: &AstNode,
    remap: &[Option<usize>],
    start: usize,
) -> Vec<usize> {
    let children = original
        .payload
        .get("children")
        .and_then(|c| c.as_array());
    match children {
        Some(arr) => arr
            .iter()
            .skip(start)
            .filter_map(|v| v.as_u64().map(|i| i as usize))
            .filter_map(|idx| remap.get(idx).copied().flatten())
            .collect(),
        None => vec![],
    }
}

/// Push a leaf node (no children) into the output arena and return its index.
fn push_leaf(out: &mut Ast, kind: &str, payload: Value) -> usize {
    out.push(AstNode::new(kind).with_payload(payload))
        .index()
}

/// Push a node with children into the output arena and return its index.
fn push_node(out: &mut Ast, kind: &str, children: Vec<usize>) -> usize {
    let payload = if children.is_empty() {
        Value::Null
    } else {
        serde_json::json!({ "children": children })
    };
    out.push(AstNode::new(kind).with_payload(payload))
        .index()
}

/// Check if `original`'s child `i` has the given kind in the original AST.
fn child_is_kind(ast: &Ast, original: &AstNode, i: usize, kind: &str) -> bool {
    let old_idx = original
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(i))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    match old_idx {
        Some(idx) => match ast.get(AstRef::new(idx)) {
            Some(child) => child.kind == kind,
            None => false,
        },
        None => false,
    }
}

// ---------------------------------------------------------------------------
// 7. `if cond then else` → `match cond { true: then, _: else }`
// ---------------------------------------------------------------------------

/// Desugar `if { children: [cond, then, else] }` into
/// `match { children: [cond, block { arm(lit(true), then), arm(var("_"), else) }] }`.
///
/// NOTE: This function is intentionally NOT registered in `try_rust_desugar`
/// because the canonical typechecker has an `if`-specific type rule
/// (`type.if`) that checks the condition is `type.bool`. Desugaring `if` to
/// `match` before typechecking would bypass this check. The function is
/// tested directly in the test module below.
#[allow(dead_code)]
fn desugar_if(
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> AstRef {
    let _ = ast;
    let cond = remap_child(original, remap, 0).unwrap_or(0);
    let then_branch = remap_child(original, remap, 1).unwrap_or(0);
    let else_branch = remap_child(original, remap, 2).unwrap_or(0);

    // arm 1: pattern = lit(true), body = then
    let true_lit = push_leaf(out, "lit", Value::Bool(true));
    let arm1 = push_node(out, "arm", vec![true_lit, then_branch]);

    // arm 2: pattern = var("_"), body = else
    let wildcard = push_leaf(out, "var", Value::from("_"));
    let arm2 = push_node(out, "arm", vec![wildcard, else_branch]);

    // block { arm1, arm2 }
    let block = push_node(out, "block", vec![arm1, arm2]);

    // match { cond, block }
    let match_idx = push_node(out, "match", vec![cond, block]);
    AstRef::new(match_idx)
}

// ---------------------------------------------------------------------------
// 4. `a ?? b` → `match a { some: value, none: b }`
// ---------------------------------------------------------------------------

/// Build an object-destructuring pattern that matches `{"_tag": "Some",
/// "value": <inner>}` and binds `binder_name` to the inner value.
///
/// Produces:
/// ```text
/// object {
///   children: [
///     entry { children: [var("_tag"), lit("Some")] },
///     entry { children: [var("value"), var(binder_name)] }
///   ]
/// }
/// ```
fn build_some_pattern(out: &mut Ast, binder_name: &str) -> usize {
    let tag_key = push_leaf(out, "var", Value::from("_tag"));
    let some_lit = push_leaf(out, "lit", Value::from("Some"));
    let entry1 = push_node(out, "entry", vec![tag_key, some_lit]);

    let value_key = push_leaf(out, "var", Value::from("value"));
    let binder = push_leaf(out, "var", Value::from(binder_name));
    let entry2 = push_node(out, "entry", vec![value_key, binder]);

    push_node(out, "object", vec![entry1, entry2])
}

/// Desugar `nullish { children: [a, b] }` into
/// `match { a, block { arm(some_pattern, binder), arm(var("_"), b) } }`.
///
/// The `some` pattern matches `{"_tag": "Some", "value": v}` and binds
/// `rec_v` to `v`. The wildcard arm catches `none`, `null`, and anything
/// else, returning `b`.
fn desugar_nullish(
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> AstRef {
    let _ = ast;
    let left = remap_child(original, remap, 0).unwrap_or(0);
    let right = remap_child(original, remap, 1).unwrap_or(0);

    // arm 1: pattern = some pattern, body = var("rec_v")
    let some_pat = build_some_pattern(out, "rec_v");
    let body_var = push_leaf(out, "var", Value::from("rec_v"));
    let arm1 = push_node(out, "arm", vec![some_pat, body_var]);

    // arm 2: pattern = var("_"), body = b
    let wildcard = push_leaf(out, "var", Value::from("_"));
    let arm2 = push_node(out, "arm", vec![wildcard, right]);

    // block { arm1, arm2 }
    let block = push_node(out, "block", vec![arm1, arm2]);

    // match { a, block }
    let match_idx = push_node(out, "match", vec![left, block]);
    AstRef::new(match_idx)
}

// ---------------------------------------------------------------------------
// 5. `a?.field` → `match a { some: v.field, none: none }`
// ---------------------------------------------------------------------------

/// Desugar `opt-chain { children: [a, field] }` into
/// `match { a, block { arm(some_pattern, field-access(rec_v, field)), arm(var("_"), lit(none)) } }`.
///
/// The `some` pattern binds `rec_v` to the inner value. The body accesses
/// `rec_v.field`. The wildcard arm returns `{"_tag": "None"}`.
fn desugar_opt_chain(
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> AstRef {
    let _ = ast;
    let left = remap_child(original, remap, 0).unwrap_or(0);
    let field = remap_child(original, remap, 1).unwrap_or(0);

    // arm 1: pattern = some pattern, body = field-access(rec_v, field)
    let some_pat = build_some_pattern(out, "rec_v");
    let rec_v_var = push_leaf(out, "var", Value::from("rec_v"));
    let field_access = push_node(out, "field-access", vec![rec_v_var, field]);
    let arm1 = push_node(out, "arm", vec![some_pat, field_access]);

    // arm 2: pattern = var("_"), body = lit({"_tag": "None"})
    let wildcard = push_leaf(out, "var", Value::from("_"));
    let none_lit = push_leaf(out, "lit", serde_json::json!({"_tag": "None"}));
    let arm2 = push_node(out, "arm", vec![wildcard, none_lit]);

    // block { arm1, arm2 }
    let block = push_node(out, "block", vec![arm1, arm2]);

    // match { a, block }
    let match_idx = push_node(out, "match", vec![left, block]);
    AstRef::new(match_idx)
}

// ---------------------------------------------------------------------------
// 1. `while cond { body }` → `rec` region
// ---------------------------------------------------------------------------

/// Desugar `while { children: [cond, body...] }` into a `rec` region with
/// feedback structure:
///
/// ```text
/// rec {
///   __continue = cond
///   __result = if __continue { body; __continue = cond } else { __result }
/// }
/// ```
///
/// **Guard**: the state-passing `while { state } { closure }` form (where
/// child 1 is a `function` node) is NOT desugared — it retains the
/// `loop_step` eval protocol. This guard distinguishes the v0.1
/// condition-based `while cond { body }` from the legacy state-passing
/// form until the parser gains contextual disambiguation.
fn desugar_while(
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Option<AstRef> {
    // Guard: skip the state-passing form (child 1 is a `function` node).
    if child_is_kind(ast, original, 1, "function") {
        return None;
    }

    let cond = remap_child(original, remap, 0).unwrap_or(0);
    let body_exprs = remap_children_from(original, remap, 1);

    // Fresh variable nodes (shared across multiple references).
    let continue_var = push_leaf(out, "var", Value::from("__continue"));
    let result_var = push_leaf(out, "var", Value::from("__result"));

    // __continue = cond
    let assign_continue = push_node(out, "assign", vec![continue_var, cond]);

    // then branch: block { body..., __continue = cond }
    let reassign = push_node(out, "assign", vec![continue_var, cond]);
    let mut block_children = body_exprs;
    block_children.push(reassign);
    let then_block = push_node(out, "block", block_children);

    // if __continue { then_block } else { __result }
    let if_node = push_node(out, "if", vec![continue_var, then_block, result_var]);

    // __result = if ...
    let assign_result = push_node(out, "assign", vec![result_var, if_node]);

    // rec { __continue = cond, __result = if ... }
    let rec_idx = push_node(out, "rec", vec![assign_continue, assign_result]);
    Some(AstRef::new(rec_idx))
}

// ---------------------------------------------------------------------------
// 2. `do { body } while cond` → `rec` region (body-first)
// ---------------------------------------------------------------------------

/// Desugar `do-while { children: [body_block, cond] }` into a `rec` region
/// that runs the body first, then checks the condition:
///
/// ```text
/// rec {
///   __result = body
///   __continue = cond
///   __result = if __continue { body; __continue = cond } else { __result }
/// }
/// ```
fn desugar_do_while(
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> AstRef {
    let _ = ast;
    let body = remap_child(original, remap, 0).unwrap_or(0);
    let cond = remap_child(original, remap, 1).unwrap_or(0);

    // Fresh variable nodes (shared across multiple references).
    let continue_var = push_leaf(out, "var", Value::from("__continue"));
    let result_var = push_leaf(out, "var", Value::from("__result"));

    // __result = body (run body first)
    let assign_result_init = push_node(out, "assign", vec![result_var, body]);

    // __continue = cond
    let assign_continue = push_node(out, "assign", vec![continue_var, cond]);

    // then branch: block { body, __continue = cond }
    let reassign = push_node(out, "assign", vec![continue_var, cond]);
    let then_block = push_node(out, "block", vec![body, reassign]);

    // if __continue { then_block } else { __result }
    let if_node = push_node(out, "if", vec![continue_var, then_block, result_var]);

    // __result = if ...
    let assign_result = push_node(out, "assign", vec![result_var, if_node]);

    // rec { __result = body, __continue = cond, __result = if ... }
    let rec_idx = push_node(
        out,
        "rec",
        vec![assign_result_init, assign_continue, assign_result],
    );
    AstRef::new(rec_idx)
}

// ---------------------------------------------------------------------------
// 3. `for x in xs { body }` → `rec` over iterator (map form)
// ---------------------------------------------------------------------------

/// Desugar `for { children: [pattern, collection, body...] }` into a `rec`
/// region that iterates over the collection:
///
/// ```text
/// rec {
///   __xs = xs
///   __result = []
///   __result = match __xs {
///     []: __result
///     [x, ...rec_tail]: block { body..., __xs = rec_tail }
///   }
/// }
/// ```
fn desugar_for(
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> AstRef {
    let _ = ast;
    let pattern = remap_child(original, remap, 0).unwrap_or(0);
    let collection = remap_child(original, remap, 1).unwrap_or(0);
    let body_exprs = remap_children_from(original, remap, 2);

    // Fresh variable nodes (shared across multiple references).
    let xs_var = push_leaf(out, "var", Value::from("__xs"));
    let result_var = push_leaf(out, "var", Value::from("__result"));
    let tail_var = push_leaf(out, "var", Value::from("rec_tail"));

    // __xs = xs
    let assign_xs = push_node(out, "assign", vec![xs_var, collection]);

    // __result = []
    let empty_array = push_node(out, "array", vec![]);
    let assign_result_init = push_node(out, "assign", vec![result_var, empty_array]);

    // match arm 1: pattern = [] (empty array), body = __result
    let empty_pat = push_node(out, "array", vec![]);
    let arm1 = push_node(out, "arm", vec![empty_pat, result_var]);

    // match arm 2: pattern = [x, ...rec_tail], body = block { body..., __xs = rec_tail }
    let rest_node = push_node(out, "rest", vec![tail_var]);
    let array_pat = push_node(out, "array", vec![pattern, rest_node]);
    let reassign_xs = push_node(out, "assign", vec![xs_var, tail_var]);
    let mut block_children = body_exprs;
    block_children.push(reassign_xs);
    let then_block = push_node(out, "block", block_children);
    let arm2 = push_node(out, "arm", vec![array_pat, then_block]);

    // match { __xs, block { arm1, arm2 } }
    let block = push_node(out, "block", vec![arm1, arm2]);
    let match_node = push_node(out, "match", vec![xs_var, block]);

    // __result = match ...
    let assign_result = push_node(out, "assign", vec![result_var, match_node]);

    // rec { __xs = xs, __result = [], __result = match ... }
    let rec_idx = push_node(
        out,
        "rec",
        vec![assign_xs, assign_result_init, assign_result],
    );
    AstRef::new(rec_idx)
}

// ---------------------------------------------------------------------------
// 8. `data Name = Cons1 | Cons2(args) | ...` → predicate + constructors + recursor
//     (ADR-0082 steps 1-2: data desugaring + strict positivity)
// ---------------------------------------------------------------------------

/// A constructor specification parsed from a `data` node's payload.
#[derive(Debug, Clone)]
struct DataConstructor {
    name: String,
    args: Vec<(String, String)>, // (arg_name, arg_type_name)
}

/// Parse a `data` node's payload into the type name and constructor list.
///
/// Expected payload shape:
/// ```json
/// {
///   "name": "Nat",
///   "constructors": [
///     { "name": "Zero", "args": [] },
///     { "name": "Succ", "args": [{"name": "pred", "type": "Nat"}] }
///   ]
/// }
/// ```
fn parse_data_payload(node: &AstNode) -> Option<(String, Vec<DataConstructor>)> {
    let name = node.payload.get("name").and_then(|v| v.as_str())?.to_string();
    let constructors = node
        .payload
        .get("constructors")
        .and_then(|c| c.as_array())?
        .iter()
        .map(|c| {
            let cname = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let args = c
                .get("args")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|a| {
                            let aname = a.get("name").and_then(|v| v.as_str()).unwrap_or("_").to_string();
                            let atype = a.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            (aname, atype)
                        })
                        .collect()
                })
                .unwrap_or_default();
            DataConstructor { name: cname, args }
        })
        .collect();
    Some((name, constructors))
}

/// Check strict positivity (ADR-0082 §6): reject a `data` declaration if a
/// constructor argument contains the type being defined in a negative
/// position (e.g. `data Bad = MkBad(Bad -> Int)`).
///
/// For simple inductive types (steps 1-2), we check that no constructor
/// argument's type name equals the type being defined when the argument
/// appears in a function position (negative). Since we only store type
/// *names* (not full type expressions) in the payload, the check is: no
/// constructor arg type may be a function type whose domain is the type
/// being defined. For the initial implementation, we check the simpler
/// condition: the type name must not appear as a function-arg type. This
/// covers the common cases (`Bad = MkBad(Bad -> Int)`, `Bad = MkBad(Int -> Bad)`).
///
/// Returns `Ok(())` if the declaration is strictly positive, `Err(reason)`
/// otherwise.
fn check_strict_positivity(type_name: &str, constructors: &[DataConstructor]) -> Result<(), String> {
    for cons in constructors {
        for (arg_name, arg_type) in &cons.args {
            // A function type containing the type name in its domain is
            // a negative occurrence. We check for the pattern `Type -> ...`
            // where `Type` is the type being defined.
            if arg_type.contains("->") {
                // The arg type is a function type. Check if the type name
                // appears in the domain (left of the first `->`).
                let domain = arg_type.split("->").next().unwrap_or("").trim();
                if domain == type_name {
                    return Err(format!(
                        "strict positivity violated: constructor '{}' arg '{}' has type '{}' \
                         with '{}' in a negative (function domain) position",
                        cons.name, arg_name, arg_type, type_name
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Desugar a `data` declaration into a `block` of definitions:
/// the predicate, each constructor, and the recursor (ADR-0082 §1).
///
/// `data Nat = Zero | Succ(Nat)` desugars to a `block` containing:
/// - `Nat = (v) -> v.tag == "Zero" or (v.tag == "Succ" and Nat(v.pred))`
/// - `Zero = { tag: "Zero" }`
/// - `Succ = (pred) -> { tag: "Succ", pred: pred }`
/// - `Nat.rec = (base, step) -> (n) -> if n.tag == "Zero" then base \
///   else step(n.pred, Nat.rec(base, step)(n.pred))`
///
/// The recursor is structurally recursive on `n` (decreasing via `n.pred`),
/// consistent with ADR-0068's `array.rec` approach.
fn desugar_data(original: &AstNode, out: &mut Ast) -> Option<AstRef> {
    let (type_name, constructors) = parse_data_payload(original)?;

    // Step 2: strict positivity check.
    if let Err(reason) = check_strict_positivity(&type_name, &constructors) {
        // On positivity violation, emit a single node that will cause a
        // runtime error when evaluated. The desugar pass is not the right
        // place to surface compile errors (it's a pure Ast→Ast transform),
        // so we encode the error as a `lit` node with an error payload.
        // The typechecker/eval will surface it.
        let err_idx = push_leaf(out, "lit", Value::from(format!("__data_error__: {}", reason)));
        return Some(AstRef::new(err_idx));
    }

    let mut stmts: Vec<usize> = Vec::new();

    // 1. Predicate: Type = (v) -> <checks>
    //    For a single constructor with no args: v.tag == "Cons"
    //    For multiple: v.tag == "C1" or (v.tag == "C2" and <recursive checks>)
    let pred_var = push_leaf(out, "var", Value::from("v"));
    let pred_body = build_predicate_body(out, &type_name, &constructors, pred_var);
    let pred_fn = push_node(out, "function", vec![pred_var, pred_body]);
    let pred_target = push_leaf(out, "var", Value::from(type_name.clone()));
    let pred_assign = push_node(out, "assign", vec![pred_target, pred_fn]);
    stmts.push(pred_assign);

    // 2. Constructors: Cons = { tag: "Cons" } or (args) -> { tag: "Cons", arg: arg, ... }
    for cons in &constructors {
        let cons_target = push_leaf(out, "var", Value::from(cons.name.clone()));
        let cons_value = if cons.args.is_empty() {
            // Nullary constructor: { tag: "ConsName" }
            push_leaf(out, "lit", serde_json::json!({ "tag": cons.name }))
        } else {
            // N-ary constructor: (arg1, arg2, ...) -> { tag: "ConsName", arg1: arg1, ... }
            let arg_vars: Vec<usize> = cons
                .args
                .iter()
                .map(|(name, _)| push_leaf(out, "var", Value::from(name.clone())))
                .collect();
            // Build the body as a `json` template node (ADR-0053/0060): the
            // template references the arg variables via `$0, $1, ...`
            // placeholders, and the json node's children are the template
            // node followed by the arg `var` nodes. Engines substitute the
            // evaluated arg values into the template at eval time, so the
            // object fields hold the actual argument values (not the arg
            // names as strings). The `json` kind is data-driven
            // (ast.yaml/eval.yaml), per the engine-genericity rule.
            let mut template = String::from("{\"tag\": ");
            template.push_str(&serde_json::to_string(&cons.name).unwrap_or_default());
            for (i, (arg_name, _)) in cons.args.iter().enumerate() {
                template.push_str(", ");
                template.push_str(&serde_json::to_string(arg_name).unwrap_or_default());
                template.push_str(": $");
                template.push_str(&i.to_string());
            }
            template.push('}');
            let template_idx = push_leaf(out, "json", Value::from(template));
            let mut body_children = vec![template_idx];
            body_children.extend(arg_vars.iter().copied());
            let body_idx = push_node(out, "json", body_children);
            let cons_fn = push_node(out, "function", {
                let mut children = arg_vars.clone();
                children.push(body_idx);
                children
            });
            cons_fn
        };
        let cons_assign = push_node(out, "assign", vec![cons_target, cons_value]);
        stmts.push(cons_assign);
    }

    // 3. Recursor: Type.rec = (base, step) -> (n) ->
    //    if n.tag == "Zero" then base
    //    else step(n.pred, Type.rec(base, step)(n.pred))
    //
    // For the general case with multiple constructors, the recursor
    // dispatches on n.tag. For the initial implementation, we generate
    // a two-constructor recursor (base + step) which covers Nat, Bool,
    // Maybe, Either patterns.
    let rec_name = format!("{}.rec", type_name);
    let rec_target = push_leaf(out, "var", Value::from(rec_name));
    let rec_fn = build_recursor(out, &type_name, &constructors);
    let rec_assign = push_node(out, "assign", vec![rec_target, rec_fn]);
    stmts.push(rec_assign);

    // Wrap all statements in a block.
    let block = push_node(out, "block", stmts);
    Some(AstRef::new(block))
}

/// Build the predicate body: a boolean expression checking `v.tag == "C1"`
/// (and recursive arg checks) `or` `v.tag == "C2"` ...
fn build_predicate_body(
    out: &mut Ast,
    type_name: &str,
    constructors: &[DataConstructor],
    v_idx: usize,
) -> usize {
    if constructors.is_empty() {
        return push_leaf(out, "lit", Value::Bool(false));
    }
    if constructors.len() == 1 {
        return build_cons_check(out, type_name, &constructors[0], v_idx);
    }
    // Multiple constructors: fold with `or`.
    let mut expr = build_cons_check(out, type_name, &constructors[0], v_idx);
    for cons in &constructors[1..] {
        let next = build_cons_check(out, type_name, cons, v_idx);
        expr = push_node(out, "binary.or", vec![expr, next]);
    }
    expr
}

/// Build a check for a single constructor: `v.tag == "ConsName"` and
/// optionally `Type(v.arg)` for each recursive arg.
fn build_cons_check(
    out: &mut Ast,
    type_name: &str,
    cons: &DataConstructor,
    v_idx: usize,
) -> usize {
    // v.tag == "ConsName"
    let tag_lit = push_leaf(out, "lit", Value::from("tag"));
    let tag_access = push_node(out, "field-access", vec![v_idx, tag_lit]);
    let cons_name_lit = push_leaf(out, "lit", Value::from(cons.name.clone()));
    let tag_eq = push_node(out, "binary.eq", vec![tag_access, cons_name_lit]);

    if cons.args.is_empty() {
        return tag_eq;
    }

    // Add recursive checks: and Type(v.arg) for each arg whose type is type_name.
    let mut expr = tag_eq;
    for (arg_name, arg_type) in &cons.args {
        if arg_type == type_name {
            // v.arg
            let arg_lit = push_leaf(out, "lit", Value::from(arg_name.clone()));
            let field_access = push_node(out, "field-access", vec![v_idx, arg_lit]);
            // Type(v.arg) — call the predicate on the field
            let type_var = push_leaf(out, "var", Value::from(type_name));
            let call = push_node(out, "call", vec![type_var, field_access]);
            expr = push_node(out, "binary.and", vec![expr, call]);
        }
    }
    expr
}

/// Build the recursor function: (base, step) -> (n) ->
///   if n.tag == "<first_cons>" then base
///   else step(n.<arg>, Type.rec(base, step)(n.<arg>))
///
/// For the initial implementation, this handles the common two-constructor
/// pattern (base + step). For types with more constructors, the recursor
/// dispatches via nested `if` on `n.tag`.
fn build_recursor(
    out: &mut Ast,
    type_name: &str,
    constructors: &[DataConstructor],
) -> usize {
    // Parameters: (base, step)
    let base_var = push_leaf(out, "var", Value::from("base"));
    let step_var = push_leaf(out, "var", Value::from("step"));

    // Inner function: (n) -> <dispatch>
    let n_var = push_leaf(out, "var", Value::from("n"));

    // Build the dispatch on n.tag for each constructor.
    let dispatch = build_recursor_dispatch(out, type_name, constructors, n_var, base_var, step_var);

    let inner_fn = push_node(out, "function", vec![n_var, dispatch]);
    // Outer function: (base, step) -> inner_fn
    push_node(out, "function", vec![base_var, step_var, inner_fn])
}

/// Build the recursor body: dispatch on n.tag.
/// - First constructor (nullary): `if n.tag == "C0" then base`
/// - Other constructors: `else if n.tag == "Ci" then step_i(n.arg, Type.rec(base, step)(n.arg))`
fn build_recursor_dispatch(
    out: &mut Ast,
    type_name: &str,
    constructors: &[DataConstructor],
    n_idx: usize,
    base_idx: usize,
    step_idx: usize,
) -> usize {
    if constructors.is_empty() {
        return push_leaf(out, "lit", Value::Null);
    }

    // n.tag
    let tag_lit = push_leaf(out, "lit", Value::from("tag"));
    let n_tag = push_node(out, "field-access", vec![n_idx, tag_lit]);

    // First constructor (base case): n.tag == "C0" → base
    let first = &constructors[0];
    let first_name_lit = push_leaf(out, "lit", Value::from(first.name.clone()));
    let first_tag_eq = push_node(out, "binary.eq", vec![n_tag, first_name_lit]);

    if constructors.len() == 1 {
        // Only one constructor — the recursor just returns base.
        let null_lit = push_leaf(out, "lit", Value::Null);
        return push_node(out, "if", vec![first_tag_eq, base_idx, null_lit]);
    }

    // Second constructor (step case): n.tag == "C1" → step(n.arg, Type.rec(base, step)(n.arg))
    let second = &constructors[1];
    let tag_lit2 = push_leaf(out, "lit", Value::from("tag"));
    let n_tag2 = push_node(out, "field-access", vec![n_idx, tag_lit2]);
    let second_name_lit = push_leaf(out, "lit", Value::from(second.name.clone()));
    let second_tag_eq = push_node(out, "binary.eq", vec![n_tag2, second_name_lit]);

    // n.arg (the recursive argument, if any)
    let step_expr = if second.args.is_empty() {
        // No args — just call step with no recursive arg.
        push_node(out, "call", vec![step_idx])
    } else {
        // n.<first_arg>
        let (arg_name, _) = &second.args[0];
        let arg_name_lit = push_leaf(out, "lit", Value::from(arg_name.clone()));
        let n_arg = push_node(out, "field-access", vec![n_idx, arg_name_lit]);
        // Type.rec(base, step) — recursive call
        let rec_var = push_leaf(out, "var", Value::from(format!("{}.rec", type_name)));
        let rec_call = push_node(out, "call", vec![rec_var, base_idx, step_idx]);
        // Type.rec(base, step)(n.arg) — apply to the recursive arg
        let rec_app = push_node(out, "call", vec![rec_call, n_arg]);
        // step(n.arg, Type.rec(base, step)(n.arg))
        push_node(out, "call", vec![step_idx, n_arg, rec_app])
    };

    // if n.tag == "C0" then base else if n.tag == "C1" then step_expr else null
    let null_lit = push_leaf(out, "lit", Value::Null);
    push_node(out, "if", vec![first_tag_eq, base_idx, second_tag_eq, step_expr, null_lit])
}

// ---------------------------------------------------------------------------
// Rust desugar dispatch
// ---------------------------------------------------------------------------

/// Try to apply a Rust-level desugar for v0.1 sugar forms.
///
/// Called after the declarative rule check fails. Returns `Some(AstRef)`
/// if a Rust desugar fired, or `None` if the node kind is not a v0.1
/// sugar form (or the guard condition skipped it).
fn try_rust_desugar(
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Option<AstRef> {
    match original.kind.as_str() {
        // NOTE: `if` → `match` desugar is implemented as `desugar_if` but
        // NOT registered in the dispatch. The canonical typechecker has
        // an `if`-specific type rule (`type.if`) that checks the condition
        // is `type.bool` — desugaring `if` to `match` before typechecking
        // would bypass this check and break
        // `typecheck_if_with_non_bool_cond_errors`. The `if` eval scheme
        // (`Seq`/`Choice`/`tag`) already handles `if` correctly at eval
        // time. The `desugar_if` function is tested directly below.
        "nullish" => Some(desugar_nullish(original, ast, out, remap)),
        "opt-chain" => Some(desugar_opt_chain(original, ast, out, remap)),
        "while" => desugar_while(original, ast, out, remap),
        "do-while" => Some(desugar_do_while(original, ast, out, remap)),
        "for" => Some(desugar_for(original, ast, out, remap)),
        "data" => desugar_data(original, out),
        _ => None,
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

    // -----------------------------------------------------------------------
    // Rust-level desugar tests for ADR-0074 v0.1 sugar forms
    // -----------------------------------------------------------------------

    /// Helper: get a child index from a node's payload.
    fn child_idx(_out: &Ast, node: &AstNode, i: usize) -> usize {
        node.payload
            .get("children")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.get(i))
            .and_then(|v| v.as_u64())
            .unwrap() as usize
    }

    /// Helper: get a child node from a node's payload.
    fn child_node<'a>(out: &'a Ast, node: &'a AstNode, i: usize) -> &'a AstNode {
        let idx = child_idx(out, node, i);
        &out.nodes()[idx]
    }

    #[test]
    fn rust_if_to_match_desugar() {
        // `if cond { then } else { else }` → `match cond { true: then, _: else }`
        // NOTE: `if` desugar is NOT registered in the dispatch (it would
        // break the typechecker's `if`-specific type rule). Test the
        // function directly by building the AST and calling `desugar_if`.
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("n"))); // 0: cond
        ast.push(AstNode::new("lit").with_payload(json!(1))); // 1: then
        ast.push(AstNode::new("lit").with_payload(json!(0))); // 2: else
        ast.push(AstNode::new("if").with_payload(json!({"children": [0, 1, 2]}))); // 3

        // Build a remap table (identity) and output arena with the children.
        let mut out = Ast::new();
        out.push(AstNode::new("var").with_payload(json!("n"))); // 0
        out.push(AstNode::new("lit").with_payload(json!(1))); // 1
        out.push(AstNode::new("lit").with_payload(json!(0))); // 2
        let remap: Vec<Option<usize>> = vec![Some(0), Some(1), Some(2), None];

        let if_node = ast.nodes().last().unwrap();
        let root_ref = desugar_if(if_node, &ast, &mut out, &remap);
        let root = out.get(root_ref).unwrap();
        assert_eq!(root.kind, "match");

        // match has children [cond, block]
        let cond = child_node(&out, root, 0);
        assert_eq!(cond.kind, "var");
        assert_eq!(cond.payload, json!("n"));

        let block = child_node(&out, root, 1);
        assert_eq!(block.kind, "block");
        assert_eq!(
            block.payload.get("children").unwrap().as_array().unwrap().len(),
            2
        );

        // arm 1: pattern = lit(true), body = then
        let arm1 = child_node(&out, block, 0);
        assert_eq!(arm1.kind, "arm");
        let pat1 = child_node(&out, arm1, 0);
        assert_eq!(pat1.kind, "lit");
        assert_eq!(pat1.payload, json!(true));
        let body1 = child_node(&out, arm1, 1);
        assert_eq!(body1.kind, "lit");
        assert_eq!(body1.payload, json!(1));

        // arm 2: pattern = var("_"), body = else
        let arm2 = child_node(&out, block, 1);
        assert_eq!(arm2.kind, "arm");
        let pat2 = child_node(&out, arm2, 0);
        assert_eq!(pat2.kind, "var");
        assert_eq!(pat2.payload, json!("_"));
        let body2 = child_node(&out, arm2, 1);
        assert_eq!(body2.kind, "lit");
        assert_eq!(body2.payload, json!(0));
    }

    #[test]
    fn rust_nullish_to_match_desugar() {
        // `a ?? b` → `match a { some: rec_v, _: b }`
        let mut lang = Language::new();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.nullish", "nullish"))
            .unwrap();

        // Build: nullish { children: [a=0, b=1] }
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("x"))); // 0: a
        ast.push(AstNode::new("lit").with_payload(json!("default"))); // 1: b
        ast.push(AstNode::new("nullish").with_payload(json!({"children": [0, 1]}))); // 2

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "match");

        // match has children [a, block]
        let left = child_node(&out, root, 0);
        assert_eq!(left.kind, "var");
        assert_eq!(left.payload, json!("x"));

        let block = child_node(&out, root, 1);
        assert_eq!(block.kind, "block");
        assert_eq!(
            block.payload.get("children").unwrap().as_array().unwrap().len(),
            2
        );

        // arm 1: pattern = object (some pattern), body = var("rec_v")
        let arm1 = child_node(&out, block, 0);
        assert_eq!(arm1.kind, "arm");
        let pat1 = child_node(&out, arm1, 0);
        assert_eq!(pat1.kind, "object");
        let body1 = child_node(&out, arm1, 1);
        assert_eq!(body1.kind, "var");
        assert_eq!(body1.payload, json!("rec_v"));

        // arm 2: pattern = var("_"), body = b
        let arm2 = child_node(&out, block, 1);
        assert_eq!(arm2.kind, "arm");
        let pat2 = child_node(&out, arm2, 0);
        assert_eq!(pat2.kind, "var");
        assert_eq!(pat2.payload, json!("_"));
        let body2 = child_node(&out, arm2, 1);
        assert_eq!(body2.kind, "lit");
        assert_eq!(body2.payload, json!("default"));
    }

    #[test]
    fn rust_opt_chain_to_match_desugar() {
        // `a?.field` → `match a { some: rec_v.field, _: none }`
        let mut lang = Language::new();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.opt-chain", "opt-chain"))
            .unwrap();

        // Build: opt-chain { children: [a=0, field=1] }
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("user"))); // 0: a
        ast.push(AstNode::new("var").with_payload(json!("name"))); // 1: field
        ast.push(AstNode::new("opt-chain").with_payload(json!({"children": [0, 1]}))); // 2

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "match");

        // match has children [a, block]
        let left = child_node(&out, root, 0);
        assert_eq!(left.kind, "var");
        assert_eq!(left.payload, json!("user"));

        let block = child_node(&out, root, 1);
        assert_eq!(block.kind, "block");

        // arm 1: pattern = object (some pattern), body = field-access
        let arm1 = child_node(&out, block, 0);
        assert_eq!(arm1.kind, "arm");
        let pat1 = child_node(&out, arm1, 0);
        assert_eq!(pat1.kind, "object");
        let body1 = child_node(&out, arm1, 1);
        assert_eq!(body1.kind, "field-access");
        // field-access has children [rec_v, field]
        let fv = child_node(&out, body1, 0);
        assert_eq!(fv.kind, "var");
        assert_eq!(fv.payload, json!("rec_v"));
        let ff = child_node(&out, body1, 1);
        assert_eq!(ff.kind, "var");
        assert_eq!(ff.payload, json!("name"));

        // arm 2: pattern = var("_"), body = lit({"_tag": "None"})
        let arm2 = child_node(&out, block, 1);
        assert_eq!(arm2.kind, "arm");
        let pat2 = child_node(&out, arm2, 0);
        assert_eq!(pat2.kind, "var");
        assert_eq!(pat2.payload, json!("_"));
        let body2 = child_node(&out, arm2, 1);
        assert_eq!(body2.kind, "lit");
        assert_eq!(body2.payload, json!({"_tag": "None"}));
    }

    #[test]
    fn rust_while_to_rec_desugar() {
        // `while cond { body }` → `rec { __continue = cond, __result = if ... }`
        let mut lang = Language::new();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.binary.gt", "binary.gt"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.assign", "assign"))
            .unwrap();
        lang.ast.merge(AstShapeDef::new("ast.while", "while")).unwrap();

        // Build: while { children: [cond=0, body=1] }
        //   0: binary.gt { children: [var("n"), lit(0)] }
        //   1: assign { children: [var("n"), binary.sub(...)] }
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("n"))); // 0
        ast.push(AstNode::new("lit").with_payload(json!(0))); // 1
        ast.push(AstNode::new("binary.gt").with_payload(json!({"children": [0, 1]}))); // 2: cond
        ast.push(AstNode::new("var").with_payload(json!("n"))); // 3
        ast.push(AstNode::new("lit").with_payload(json!(1))); // 4
        ast.push(AstNode::new("assign").with_payload(json!({"children": [3, 4]}))); // 5: body
        ast.push(AstNode::new("while").with_payload(json!({"children": [2, 5]}))); // 6

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "rec");

        // rec has 2 children: [assign(__continue, cond), assign(__result, if)]
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 2);

        // child 0: assign(__continue, cond)
        let assign0 = &out.nodes()[children[0].as_u64().unwrap() as usize];
        assert_eq!(assign0.kind, "assign");
        let a0_var = child_node(&out, assign0, 0);
        assert_eq!(a0_var.payload, json!("__continue"));
        let a0_val = child_node(&out, assign0, 1);
        assert_eq!(a0_val.kind, "binary.gt");

        // child 1: assign(__result, if ...)
        let assign1 = &out.nodes()[children[1].as_u64().unwrap() as usize];
        assert_eq!(assign1.kind, "assign");
        let a1_var = child_node(&out, assign1, 0);
        assert_eq!(a1_var.payload, json!("__result"));
        let a1_val = child_node(&out, assign1, 1);
        assert_eq!(a1_val.kind, "if");

        // if has 3 children: [__continue, block, __result]
        let if_cond = child_node(&out, a1_val, 0);
        assert_eq!(if_cond.payload, json!("__continue"));
        let if_then = child_node(&out, a1_val, 1);
        assert_eq!(if_then.kind, "block");
        let if_else = child_node(&out, a1_val, 2);
        assert_eq!(if_else.payload, json!("__result"));

        // block contains body + reassign(__continue, cond)
        let block_children = if_then.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(block_children.len(), 2);
        let block_body = &out.nodes()[block_children[0].as_u64().unwrap() as usize];
        assert_eq!(block_body.kind, "assign");
        let block_reassign = &out.nodes()[block_children[1].as_u64().unwrap() as usize];
        assert_eq!(block_reassign.kind, "assign");
        let reassign_var = child_node(&out, block_reassign, 0);
        assert_eq!(reassign_var.payload, json!("__continue"));
    }

    #[test]
    fn rust_while_state_passing_not_desugared() {
        // The state-passing `while { state } { closure }` form (child 1 is
        // a `function` node) should NOT be desugared — it retains the
        // `loop_step` eval protocol.
        let mut lang = Language::new();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.object", "object")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.function", "function"))
            .unwrap();
        lang.ast.merge(AstShapeDef::new("ast.while", "while")).unwrap();

        // Build: while { children: [object=0, function=1] }
        let mut ast = Ast::new();
        ast.push(AstNode::new("object").with_payload(json!({"children": []}))); // 0: state
        ast.push(AstNode::new("function").with_payload(json!({"children": [0, 0]}))); // 1: closure
        ast.push(AstNode::new("while").with_payload(json!({"children": [0, 1]}))); // 2

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        // Should remain `while`, not desugared to `rec`.
        assert_eq!(root.kind, "while");
    }

    #[test]
    fn rust_do_while_to_rec_desugar() {
        // `do { body } while cond` → `rec { __result = body, __continue = cond, __result = if ... }`
        let mut lang = Language::new();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.block", "block")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.binary.gt", "binary.gt"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.do-while", "do-while"))
            .unwrap();

        // Build: do-while { children: [body=0, cond=1] }
        let mut ast = Ast::new();
        ast.push(AstNode::new("block").with_payload(json!({"children": []}))); // 0: body
        ast.push(AstNode::new("var").with_payload(json!("n"))); // 1
        ast.push(AstNode::new("lit").with_payload(json!(0))); // 2
        ast.push(AstNode::new("binary.gt").with_payload(json!({"children": [1, 2]}))); // 3: cond
        ast.push(AstNode::new("do-while").with_payload(json!({"children": [0, 3]}))); // 4

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "rec");

        // rec has 3 children: [assign(__result, body), assign(__continue, cond), assign(__result, if)]
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 3);

        // child 0: assign(__result, body)
        let assign0 = &out.nodes()[children[0].as_u64().unwrap() as usize];
        assert_eq!(assign0.kind, "assign");
        let a0_var = child_node(&out, assign0, 0);
        assert_eq!(a0_var.payload, json!("__result"));
        let a0_val = child_node(&out, assign0, 1);
        assert_eq!(a0_val.kind, "block");

        // child 1: assign(__continue, cond)
        let assign1 = &out.nodes()[children[1].as_u64().unwrap() as usize];
        assert_eq!(assign1.kind, "assign");
        let a1_var = child_node(&out, assign1, 0);
        assert_eq!(a1_var.payload, json!("__continue"));
        let a1_val = child_node(&out, assign1, 1);
        assert_eq!(a1_val.kind, "binary.gt");

        // child 2: assign(__result, if ...)
        let assign2 = &out.nodes()[children[2].as_u64().unwrap() as usize];
        assert_eq!(assign2.kind, "assign");
        let a2_val = child_node(&out, assign2, 1);
        assert_eq!(a2_val.kind, "if");
    }

    #[test]
    fn rust_for_to_rec_desugar() {
        // `for x in xs { body }` → `rec { __xs = xs, __result = [], __result = match ... }`
        let mut lang = Language::new();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.binary.mul", "binary.mul"))
            .unwrap();
        lang.ast.merge(AstShapeDef::new("ast.for", "for")).unwrap();

        // Build: for { children: [pattern=0, collection=1, body=2] }
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("x"))); // 0: pattern
        ast.push(AstNode::new("var").with_payload(json!("xs"))); // 1: collection
        ast.push(AstNode::new("var").with_payload(json!("x"))); // 2
        ast.push(AstNode::new("lit").with_payload(json!(2))); // 3
        ast.push(AstNode::new("binary.mul").with_payload(json!({"children": [2, 3]}))); // 4: body
        ast.push(AstNode::new("for").with_payload(json!({"children": [0, 1, 4]}))); // 5

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "rec");

        // rec has 3 children: [assign(__xs, xs), assign(__result, []), assign(__result, match)]
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 3);

        // child 0: assign(__xs, collection)
        let assign0 = &out.nodes()[children[0].as_u64().unwrap() as usize];
        assert_eq!(assign0.kind, "assign");
        let a0_var = child_node(&out, assign0, 0);
        assert_eq!(a0_var.payload, json!("__xs"));
        let a0_val = child_node(&out, assign0, 1);
        assert_eq!(a0_val.kind, "var");
        assert_eq!(a0_val.payload, json!("xs"));

        // child 1: assign(__result, [])
        let assign1 = &out.nodes()[children[1].as_u64().unwrap() as usize];
        assert_eq!(assign1.kind, "assign");
        let a1_var = child_node(&out, assign1, 0);
        assert_eq!(a1_var.payload, json!("__result"));
        let a1_val = child_node(&out, assign1, 1);
        assert_eq!(a1_val.kind, "array");

        // child 2: assign(__result, match ...)
        let assign2 = &out.nodes()[children[2].as_u64().unwrap() as usize];
        assert_eq!(assign2.kind, "assign");
        let a2_val = child_node(&out, assign2, 1);
        assert_eq!(a2_val.kind, "match");

        // match has 2 children: [__xs, block]
        let match_scrut = child_node(&out, a2_val, 0);
        assert_eq!(match_scrut.payload, json!("__xs"));
        let match_block = child_node(&out, a2_val, 1);
        assert_eq!(match_block.kind, "block");

        // block has 2 arms
        let block_children = match_block
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(block_children.len(), 2);

        // arm 1: pattern = [] (empty array), body = __result
        let arm1 = &out.nodes()[block_children[0].as_u64().unwrap() as usize];
        assert_eq!(arm1.kind, "arm");
        let arm1_pat = child_node(&out, arm1, 0);
        assert_eq!(arm1_pat.kind, "array");
        let arm1_body = child_node(&out, arm1, 1);
        assert_eq!(arm1_body.payload, json!("__result"));

        // arm 2: pattern = [x, ...rec_tail], body = block { body, __xs = rec_tail }
        let arm2 = &out.nodes()[block_children[1].as_u64().unwrap() as usize];
        assert_eq!(arm2.kind, "arm");
        let arm2_pat = child_node(&out, arm2, 0);
        assert_eq!(arm2_pat.kind, "array");
        // pattern has [x, rest]
        let pat_children = arm2_pat
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(pat_children.len(), 2);
        let pat_x = &out.nodes()[pat_children[0].as_u64().unwrap() as usize];
        assert_eq!(pat_x.payload, json!("x"));
        let pat_rest = &out.nodes()[pat_children[1].as_u64().unwrap() as usize];
        assert_eq!(pat_rest.kind, "rest");
        // arm 2 body is a block
        let arm2_body = child_node(&out, arm2, 1);
        assert_eq!(arm2_body.kind, "block");
    }

    #[test]
    fn rust_desugar_nested_nullish() {
        // Nested `nullish` — both should desugar to `match`.
        let mut lang = Language::new();
        lang.ast.merge(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast.merge(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.nullish", "nullish"))
            .unwrap();

        // Build: nullish { children: [inner_nullish=3, lit=4] }
        //   0: var("a")     (inner left)
        //   1: lit("b")     (inner right)
        //   2: nullish { children: [0, 1] }  (inner nullish)
        //   3: lit("default")  (outer right)
        //   4: nullish { children: [2, 3] }  (outer nullish)
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("a"))); // 0
        ast.push(AstNode::new("lit").with_payload(json!("b"))); // 1
        ast.push(AstNode::new("nullish").with_payload(json!({"children": [0, 1]}))); // 2
        ast.push(AstNode::new("lit").with_payload(json!("default"))); // 3
        ast.push(AstNode::new("nullish").with_payload(json!({"children": [2, 3]}))); // 4

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        // outer nullish → match
        assert_eq!(root.kind, "match");

        // The outer's left child (originally inner nullish) should also be
        // desugared to match.
        let left = child_node(&out, root, 0);
        assert_eq!(
            left.kind, "match",
            "nested nullish should be desugared to match"
        );
    }

    // -----------------------------------------------------------------------
    // ADR-0082 step 1-2: `data` desugaring + strict positivity
    // -----------------------------------------------------------------------

    /// Helper: build a `data` AST node with the given type name and
    /// constructors.
    fn data_node(name: &str, constructors: &[(&str, &[(&str, &str)])]) -> AstNode {
        let cons: Vec<serde_json::Value> = constructors
            .iter()
            .map(|(cname, args)| {
                let args_arr: Vec<serde_json::Value> = args
                    .iter()
                    .map(|(an, at)| json!({"name": an, "type": at}))
                    .collect();
                json!({"name": cname, "args": args_arr})
            })
            .collect();
        AstNode::new("data").with_payload(json!({
            "name": name,
            "constructors": cons
        }))
    }

    #[test]
    fn data_nat_desugars_to_block() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node("Nat", &[("Zero", &[]), ("Succ", &[("pred", "Nat")])]));
        let out = apply_desugar(&lang, &ast);

        // The root should be a `block` containing the predicate, two
        // constructors, and the recursor (4 assign statements).
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 4, "expected 4 statements (pred + 2 cons + rec)");

        // Each statement should be an `assign`.
        for (i, child_idx) in children.iter().enumerate() {
            let node = out.get(AstRef::new(child_idx.as_u64().unwrap() as usize)).unwrap();
            assert_eq!(node.kind, "assign", "statement {} should be assign, got {}", i, node.kind);
        }
    }

    #[test]
    fn data_bool_desugars_correctly() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node("Bool", &[("True", &[]), ("False", &[])]));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // 1 predicate + 2 constructors + 1 recursor = 4
        assert_eq!(children.len(), 4);
    }

    #[test]
    fn data_strict_positivity_violation() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        // data Bad = MkBad(Bad -> Int) — negative occurrence
        ast.push(data_node("Bad", &[("MkBad", &[("f", "Bad -> Int")])]));
        let out = apply_desugar(&lang, &ast);

        // Should produce a `lit` node with an error message, not a `block`.
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "lit");
        let msg = root.payload.as_str().unwrap_or("");
        assert!(
            msg.contains("__data_error__"),
            "expected data error, got: {}",
            msg
        );
        assert!(
            msg.contains("strict positivity"),
            "expected strict positivity error, got: {}",
            msg
        );
    }

    #[test]
    fn data_maybe_desugars_correctly() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        // data Maybe = None | Some(value)
        // (value is untyped — not recursive, so no recursive check)
        ast.push(data_node("Maybe", &[("None", &[]), ("Some", &[("value", "Int")])]));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // 1 predicate + 2 constructors + 1 recursor = 4
        assert_eq!(children.len(), 4);
    }

    #[test]
    fn data_nat_predicate_is_function() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node("Nat", &[("Zero", &[]), ("Succ", &[("pred", "Nat")])]));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();

        // First statement: Nat = (v) -> ...  (assign with var target, function value)
        let pred_assign = out.get(AstRef::new(children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(pred_assign.kind, "assign");
        let pred_children = pred_assign.payload.get("children").unwrap().as_array().unwrap();
        // Target: var("Nat")
        let target = out.get(AstRef::new(pred_children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(target.kind, "var");
        assert_eq!(target.payload, json!("Nat"));
        // Value: function
        let value = out.get(AstRef::new(pred_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(value.kind, "function");
    }

    #[test]
    fn data_nat_recursor_is_function() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node("Nat", &[("Zero", &[]), ("Succ", &[("pred", "Nat")])]));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        let children = root.payload.get("children").unwrap().as_array().unwrap();

        // Last statement: Nat.rec = (base, step) -> (n) -> ...
        let rec_assign = out.get(AstRef::new(children[3].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(rec_assign.kind, "assign");
        let rec_children = rec_assign.payload.get("children").unwrap().as_array().unwrap();
        // Target: var("Nat.rec")
        let target = out.get(AstRef::new(rec_children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(target.kind, "var");
        assert_eq!(target.payload, json!("Nat.rec"));
        // Value: function (outer: base, step)
        let value = out.get(AstRef::new(rec_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(value.kind, "function");
    }
}
