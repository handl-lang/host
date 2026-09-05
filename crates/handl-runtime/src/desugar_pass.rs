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

/// Push a `function` node with the given parameter names and body index.
/// Uses the rich payload format (`params` + `body`) which the `closure`
/// eval primitive reads directly. This correctly supports multi-parameter
/// functions, unlike the `children` array format which only supports a
/// single param node at `children[0]`.
fn push_function(out: &mut Ast, params: &[&str], body_idx: usize) -> usize {
    let params_arr: Vec<Value> = params.iter().map(|s| Value::from(*s)).collect();
    let payload = serde_json::json!({
        "params": params_arr,
        "body": body_idx
    });
    out.push(AstNode::new("function").with_payload(payload))
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
/// Supports two payload shapes:
///
/// **Hand-built (backward compat):** structured JSON payload:
/// ```json
/// {
///   "name": "Nat",
///   "constructors": [
///     { "name": "Zero", "args": [] },
///     { "name": "Succ", "args": [{"name": "pred", "type": "Nat"}] }
///   ]
/// }
/// ```
///
/// **Parser-produced (ADR-0082):** `payload.name` (string from
/// `payload_tokens`) + one child (the constructor list expression). The
/// child is an `alt` tree (from `|` pipe infix) or a single constructor
/// node. Each constructor is either:
/// - A `var` node: nullary constructor (name = var payload, no args).
/// - A `call` node: N-ary constructor (callee child 0 is a `var` with the
///   name; children 1+ are type expressions, each a `var` whose payload is
///   the type name). Arg names are derived from the lowercased type name.
fn parse_data_payload(node: &AstNode, ast: &Ast) -> Option<(String, Vec<DataConstructor>)> {
    let name = node.payload.get("name").and_then(|v| v.as_str())?.to_string();

    // Backward compatibility: if `payload.constructors` exists (hand-built
    // test nodes), use the old structured-JSON path.
    if let Some(constructors) = node.payload.get("constructors").and_then(|c| c.as_array()) {
        let cons = constructors
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
        return Some((name, cons));
    }

    // Parser-produced path: extract constructors from the child AST.
    // The `data` node has one child (the constructor list expression).
    let child_idx = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)?;
    let child = ast.get(AstRef::new(child_idx))?;
    let mut constructors = Vec::new();
    collect_constructors(child, ast, &mut constructors);
    Some((name, constructors))
}

/// Recursively collect constructors from an `alt` tree or a single
/// constructor node.
///
/// - `alt` node: recurse on both children (left and right).
/// - `var` node: nullary constructor (name = payload string, no args).
/// - `call` node: N-ary constructor (callee = child 0 `var`, args =
///   children 1+ type expressions). Arg names are derived from the
///   lowercased type name; arg types from the `var` payload. Non-`var`
///   args get a placeholder type `"_"`.
fn collect_constructors(node: &AstNode, ast: &Ast, out: &mut Vec<DataConstructor>) {
    match node.kind.as_str() {
        "alt" => {
            // Recurse on both children.
            let children = node.payload.get("children").and_then(|c| c.as_array());
            if let Some(arr) = children {
                for child_val in arr {
                    if let Some(idx) = child_val.as_u64() {
                        if let Some(child) = ast.get(AstRef::new(idx as usize)) {
                            collect_constructors(child, ast, out);
                        }
                    }
                }
            }
        }
        "var" => {
            // Nullary constructor: name = payload string, no args.
            let cname = node.payload.as_str().unwrap_or("").to_string();
            out.push(DataConstructor {
                name: cname,
                args: Vec::new(),
            });
        }
        "call" => {
            // N-ary constructor: callee (child 0) is a `var` with the name,
            // children 1+ are type expression args.
            let children = node.payload.get("children").and_then(|c| c.as_array());
            let arr = match children {
                Some(a) => a,
                None => return,
            };
            // Child 0: callee (var with constructor name).
            let callee_idx = arr.first().and_then(|v| v.as_u64()).map(|v| v as usize);
            let cname = callee_idx
                .and_then(|idx| ast.get(AstRef::new(idx)))
                .and_then(|n| n.payload.as_str())
                .unwrap_or("")
                .to_string();
            // Children 1+: type expression args. Each arg is either:
            // - A `var` node: implicit arg name (lowercased type name), e.g.
            //   `Succ(Nat)` → arg name "nat", type "Nat".
            // - A `type-annot` node: explicit arg name, e.g.
            //   `Just(value : Nat)` → arg name "value", type "Nat". The
            //   `type-annot` node has children [subject(var), annot(var)].
            let mut args = Vec::new();
            for (i, arg_val) in arr.iter().enumerate().skip(1) {
                if let Some(arg_idx) = arg_val.as_u64() {
                    if let Some(arg_node) = ast.get(AstRef::new(arg_idx as usize)) {
                        let (arg_name, arg_type) = if arg_node.kind == "var" {
                            // Implicit arg: `Succ(Nat)` → name "nat", type "Nat".
                            let type_name = arg_node.payload.as_str().unwrap_or("_").to_string();
                            let lower = type_name.to_lowercase();
                            (lower, type_name)
                        } else if arg_node.kind == "type-annot" {
                            // Explicit arg: `Just(value : Nat)` → name "value",
                            // type "Nat". The type-annot node has children
                            // [subject, annot] where subject is a var with the
                            // arg name and annot is a var with the type name.
                            let ta_children = arg_node
                                .payload
                                .get("children")
                                .and_then(|c| c.as_array());
                            let (name, typ) = match ta_children {
                                Some(ta_arr) if ta_arr.len() >= 2 => {
                                    let subject_name = ta_arr[0]
                                        .as_u64()
                                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                                        .and_then(|n| n.payload.as_str())
                                        .unwrap_or("_")
                                        .to_string();
                                    let type_name = ta_arr[1]
                                        .as_u64()
                                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                                        .and_then(|n| n.payload.as_str())
                                        .unwrap_or("_")
                                        .to_string();
                                    (subject_name, type_name)
                                }
                                _ => (format!("_{}", i - 1), "_".to_string()),
                            };
                            (name, typ)
                        } else {
                            // Non-var, non-type-annot: use placeholder.
                            (format!("_{}", i - 1), "_".to_string())
                        };
                        args.push((arg_name, arg_type));
                    }
                }
            }
            out.push(DataConstructor { name: cname, args });
        }
        _ => {
            // Unknown node kind: skip (or could be a single constructor).
        }
    }
}

/// Check strict positivity (ADR-0082 §6): reject a `data` declaration if a
/// constructor argument contains the type being defined in a negative
/// position (e.g. `data Bad = MkBad(Bad -> Int)`).
///
/// This parses each constructor argument's type expression into a tree and
/// walks it with the Coq-style strict positivity rule: in `A -> B`, the type
/// being defined must not occur **at all** in `A` (the domain), and must be
/// strictly positive in `B` (the codomain). In `Name(args)`, the type may
/// occur in `args` (all positive). A bare `Name` is always strictly positive.
///
/// This handles nested function types (`(Bad -> Int) -> Int`), parameterized
/// types (`List(Bad)`), and parenthesized types.
///
/// Returns `Ok(())` if the declaration is strictly positive, `Err(reason)`
/// otherwise.
fn check_strict_positivity(type_name: &str, constructors: &[DataConstructor]) -> Result<(), String> {
    for cons in constructors {
        for (arg_name, arg_type) in &cons.args {
            let mut parser = TypeExprParser::new(arg_type);
            if let Some(expr) = parser.parse() {
                if let Err(reason) = check_strict_positivity_rec(&expr, type_name) {
                    return Err(format!(
                        "strict positivity violated: constructor '{}' arg '{}' has type '{}' \
                         — {}",
                        cons.name, arg_name, arg_type, reason
                    ));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Type-expression parser for strict positivity checking
// ---------------------------------------------------------------------------

/// A parsed type expression.
#[derive(Debug, Clone)]
enum TypeExpr {
    /// A bare type name, e.g. `Nat`.
    Atom(String),
    /// A parameterized type application, e.g. `List(Nat)`.
    App(String, Vec<TypeExpr>),
    /// A function type `A -> B`.
    Func(Box<TypeExpr>, Box<TypeExpr>),
}

/// Tokenize a type-expression string into `->`, `(`, `)`, `,`, and
/// identifiers (alphanumeric + `_`).
fn tokenize_type_expr(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '-' && chars.clone().nth(1) == Some('>') {
            chars.next();
            chars.next();
            tokens.push("->".to_string());
        } else if c == '(' || c == ')' || c == ',' {
            tokens.push(c.to_string());
            chars.next();
        } else if c.is_alphanumeric() || c == '_' {
            let mut ident = String::new();
            while let Some(&c2) = chars.peek() {
                if c2.is_alphanumeric() || c2 == '_' {
                    ident.push(c2);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(ident);
        } else {
            // Unknown character — skip it.
            chars.next();
        }
    }
    tokens
}

/// A simple recursive-descent parser for type expressions.
///
/// Grammar:
/// ```text
/// TypeExpr     = FuncType
/// FuncType     = AppType ('->' FuncType)?
/// AppType      = AtomType ('(' TypeExprList ')')?
/// AtomType     = identifier | '(' FuncType ')'
/// TypeExprList = FuncType (',' FuncType)*
/// ```
struct TypeExprParser {
    tokens: Vec<String>,
    pos: usize,
}

impl TypeExprParser {
    fn new(s: &str) -> Self {
        TypeExprParser {
            tokens: tokenize_type_expr(s),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|s| s.as_str())
    }

    fn advance(&mut self) -> Option<&str> {
        let tok = self.tokens.get(self.pos).map(|s| s.as_str());
        self.pos += 1;
        tok
    }

    fn parse(&mut self) -> Option<TypeExpr> {
        self.parse_func_type()
    }

    fn parse_func_type(&mut self) -> Option<TypeExpr> {
        let left = self.parse_app_type()?;
        if self.peek() == Some("->") {
            self.advance(); // consume `->`
            let right = self.parse_func_type()?;
            Some(TypeExpr::Func(Box::new(left), Box::new(right)))
        } else {
            Some(left)
        }
    }

    fn parse_app_type(&mut self) -> Option<TypeExpr> {
        let atom = self.parse_atom_type()?;
        // Check for `(args)` — a parameterized type application.
        if self.peek() == Some("(") {
            self.advance(); // consume `(`
            let mut args = Vec::new();
            if self.peek() != Some(")") {
                args.push(self.parse_func_type()?);
                while self.peek() == Some(",") {
                    self.advance(); // consume `,`
                    args.push(self.parse_func_type()?);
                }
            }
            if self.peek() == Some(")") {
                self.advance(); // consume `)`
            }
            if let TypeExpr::Atom(name) = atom {
                Some(TypeExpr::App(name, args))
            } else {
                // A parenthesized type followed by `(args)` — unusual; just
                // return the atom.
                Some(atom)
            }
        } else {
            Some(atom)
        }
    }

    fn parse_atom_type(&mut self) -> Option<TypeExpr> {
        let peeked = self.peek().map(|s| s.to_string());
        match peeked.as_deref() {
            Some("(") => {
                self.advance(); // consume `(`
                let inner = self.parse_func_type()?;
                if self.peek() == Some(")") {
                    self.advance(); // consume `)`
                }
                // Parentheses just unwrap — return the inner type directly.
                Some(inner)
            }
            Some(tok) if tok != "->" && tok != ")" && tok != "," => {
                self.advance();
                Some(TypeExpr::Atom(tok.to_string()))
            }
            _ => None,
        }
    }
}

/// Recursively check strict positivity: the type being defined must not
/// occur in the domain of any function type (Coq-style strict positivity).
///
/// - `Atom(name)`: always OK (a bare type name is in a positive position).
/// - `App(name, args)`: recurse into each arg (all positive positions).
/// - `Func(domain, codomain)`: the type must not occur **at all** in
///   `domain`; it must be strictly positive in `codomain`.
fn check_strict_positivity_rec(expr: &TypeExpr, type_name: &str) -> Result<(), String> {
    match expr {
        TypeExpr::Atom(_) => Ok(()),
        TypeExpr::App(_, args) => {
            for arg in args {
                check_strict_positivity_rec(arg, type_name)?;
            }
            Ok(())
        }
        TypeExpr::Func(domain, codomain) => {
            if occurs_in(domain, type_name) {
                Err(format!(
                    "'{}' occurs in a function domain (negative position)",
                    type_name
                ))
            } else {
                check_strict_positivity_rec(codomain, type_name)
            }
        }
    }
}

/// Check whether the type name occurs anywhere in a type expression.
fn occurs_in(expr: &TypeExpr, type_name: &str) -> bool {
    match expr {
        TypeExpr::Atom(name) => name == type_name,
        TypeExpr::App(name, args) => {
            name == type_name || args.iter().any(|a| occurs_in(a, type_name))
        }
        TypeExpr::Func(domain, codomain) => {
            occurs_in(domain, type_name) || occurs_in(codomain, type_name)
        }
    }
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
fn desugar_data(original: &AstNode, ast: &Ast, out: &mut Ast) -> Option<AstRef> {
    let (type_name, constructors) = parse_data_payload(original, ast)?;

    // Step 2: strict positivity check.
    if let Err(reason) = check_strict_positivity(&type_name, &constructors) {
        // On positivity violation, emit a `desugar-error` node. The
        // `desugar-error` node kind is data-driven (declared in
        // ast.yaml/eval.yaml). Its eval scheme raises a `LangError` when
        // evaluated, so the error surfaces at eval time instead of being
        // silently swallowed as a string literal.
        let err_payload = serde_json::json!({
            "kind": "data-error",
            "reason": reason
        });
        let err_idx = push_leaf(out, "desugar-error", err_payload);
        return Some(AstRef::new(err_idx));
    }

    let mut stmts: Vec<usize> = Vec::new();

    // 1. Predicate: Type = (v) -> <checks>
    //    For a single constructor with no args: v.tag == "Cons"
    //    For multiple: v.tag == "C1" or (v.tag == "C2" and <recursive checks>)
    let pred_var = push_leaf(out, "var", Value::from("v"));
    let pred_body = build_predicate_body(out, &type_name, &constructors, pred_var);
    let pred_fn = push_function(out, &["v"], pred_body);
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
            let param_names: Vec<&str> = cons.args.iter().map(|(n, _)| n.as_str()).collect();
            let cons_fn = push_function(out, &param_names, body_idx);
            cons_fn
        };
        let cons_assign = push_node(out, "assign", vec![cons_target, cons_value]);
        stmts.push(cons_assign);
    }

    // 3. Recursor: Type.rec = (base, step) -> (n) ->
    //    if n.tag == "Zero" then base
    //    else step(n.pred, Type.rec(base, step)(n.pred))
    //
    // The recursor needs to reference itself recursively. A plain `assign`
    // is non-recursive (the binding is not in scope inside its own body).
    // We use a `rec` region with a self-application (Y-combinator) technique:
    //
    //   rec {
    //     __rec_inner = (self) -> (base, step) -> (n) ->
    //       if n.tag == "Zero" then base
    //       else step(n.pred, self(self)(base, step)(n.pred))
    //     Type.rec = __rec_inner(__rec_inner)
    //   }
    //
    // The `rec` region allows the bindings to see each other. The
    // self-application `__rec_inner(__rec_inner)` binds `self` to
    // `__rec_inner`, so the recursive call `self(self)` re-applies
    // `__rec_inner` to itself, creating a fresh closure at each recursion
    // level. This works for arbitrary recursion depth.
    //
    // The `rec` region's result (the value of the last binding,
    // `Type.rec = __rec_inner(__rec_inner)`) is the recursor closure. We
    // wrap the `rec` in an `assign` so `Type.rec` is defined in the
    // enclosing block scope.
    let rec_name = format!("{}.rec", type_name);
    let rec_target = push_leaf(out, "var", Value::from(rec_name.clone()));
    let rec_inner_fn = build_recursor(out, &type_name, &constructors);

    // rec region: { __rec_inner = inner_fn, Type.rec = __rec_inner(__rec_inner) }
    let inner_name = format!("__rec_inner_{}", type_name);
    let inner_target = push_leaf(out, "var", Value::from(inner_name.clone()));
    let inner_assign = push_node(out, "assign", vec![inner_target, rec_inner_fn]);

    // Type.rec = __rec_inner(__rec_inner)
    let inner_var1 = push_leaf(out, "var", Value::from(inner_name.clone()));
    let inner_var2 = push_leaf(out, "var", Value::from(inner_name));
    let self_app = push_node(out, "call", vec![inner_var1, inner_var2]);
    let rec_inner_target = push_leaf(out, "var", Value::from(rec_name));
    let rec_inner_assign = push_node(out, "assign", vec![rec_inner_target, self_app]);

    let rec_region = push_node(out, "rec", vec![inner_assign, rec_inner_assign]);

    // assign { var("Type.rec"), rec { ... } }
    let rec_assign = push_node(out, "assign", vec![rec_target, rec_region]);
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

/// Build the recursor inner function (self-application form):
///   (self) -> (c0, c1, ..., c_{N-1}) -> (n) ->
///     if n.tag == "C0" then <c0-branch>
///     else if n.tag == "C1" then <c1-branch>
///     ...
///     else null
///
/// The recursor takes one parameter per constructor. For a nullary
/// constructor `Ci`, the branch returns `ci` directly. For a non-nullary
/// constructor `Ci(a1, ..., ak)`, the branch calls `ci(n.a1, ..., n.ak,
/// rec_1, ..., rec_m)` where `rec_j` is the recursive result for each
/// recursive arg (arg whose type equals the type being defined).
///
/// The `self` parameter is the recursor's own inner function. The recursive
/// call uses `self(self)(c0, ..., c_{N-1})(n.arg)` instead of naming the
/// recursor directly, enabling recursion via self-application (Y-combinator
/// style). This avoids the need for a self-referential binding.
fn build_recursor(
    out: &mut Ast,
    type_name: &str,
    constructors: &[DataConstructor],
) -> usize {
    // Parameter: self (the inner function, applied to itself for recursion)
    let self_var = push_leaf(out, "var", Value::from("self"));

    // Generate one parameter per constructor: c0, c1, ..., c_{N-1}.
    let param_names: Vec<String> = (0..constructors.len()).map(|i| format!("c{}", i)).collect();
    let param_vars: Vec<usize> = param_names
        .iter()
        .map(|name| push_leaf(out, "var", Value::from(name.clone())))
        .collect();

    // Inner function: (n) -> <dispatch>
    let n_var = push_leaf(out, "var", Value::from("n"));

    // Build the dispatch on n.tag for each constructor.
    let dispatch = build_recursor_dispatch(out, type_name, constructors, n_var, &param_vars, self_var);

    let inner_fn = push_function(out, &["n"], dispatch);
    // Middle function: (c0, c1, ..., c_{N-1}) -> inner_fn
    let mid_param_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();
    let mid_fn = push_function(out, &mid_param_refs, inner_fn);
    // Outer function: (self) -> mid_fn
    push_function(out, &["self"], mid_fn)
}

/// Build the recursor body: dispatch on `n.tag` across all constructors,
/// building a right-nested `if` tree.
///
/// For each constructor `Ci`:
/// - **Nullary** (`Ci` with no args): the branch is `ci` (the parameter
///   returned directly).
/// - **Non-nullary** (`Ci(a1, ..., ak)`): the branch is
///   `ci(n.a1, ..., n.ak, rec_1, ..., rec_m)` where `rec_j` is
///   `self(self)(c0, ..., c_{N-1})(n.aj)` for each recursive arg `aj`
///   (whose type equals `type_name`).
///
/// The `if` eval scheme expects exactly 3 children `[cond, then, else]`,
/// so we nest the else-if chain right-to-left, with `null` as the
/// final fallback.
fn build_recursor_dispatch(
    out: &mut Ast,
    type_name: &str,
    constructors: &[DataConstructor],
    n_idx: usize,
    param_vars: &[usize],
    self_idx: usize,
) -> usize {
    if constructors.is_empty() {
        return push_leaf(out, "lit", Value::Null);
    }

    // Build `self(self)` — the recursive self-application. Shared across
    // all recursive calls (multiple nodes can reference the same arena index).
    let self_self = push_node(out, "call", vec![self_idx, self_idx]);
    // Build `self(self)(c0, c1, ..., c_{N-1})` — the recursor closure,
    // shared across all recursive calls.
    let mut rec_app_children = vec![self_self];
    rec_app_children.extend(param_vars.iter().copied());
    let rec_app = push_node(out, "call", rec_app_children);

    // Fold right-to-left over constructors, building a right-nested if tree.
    // The last constructor's else branch is `null`.
    let mut else_branch = push_leaf(out, "lit", Value::Null);

    for i in (0..constructors.len()).rev() {
        let cons = &constructors[i];

        // n.tag == "Ci"
        let tag_lit = push_leaf(out, "lit", Value::from("tag"));
        let n_tag = push_node(out, "field-access", vec![n_idx, tag_lit]);
        let cons_name_lit = push_leaf(out, "lit", Value::from(cons.name.clone()));
        let tag_eq = push_node(out, "binary.eq", vec![n_tag, cons_name_lit]);

        // Build the then-branch for constructor Ci.
        let then_branch = if cons.args.is_empty() {
            // Nullary constructor: return the param directly.
            param_vars[i]
        } else {
            // Non-nullary: call ci(n.a1, ..., n.ak, rec_1, ..., rec_m)
            let mut call_children = vec![param_vars[i]];
            // Pass all constructor args: n.a1, n.a2, ...
            for (arg_name, _) in &cons.args {
                let arg_name_lit = push_leaf(out, "lit", Value::from(arg_name.clone()));
                let n_arg = push_node(out, "field-access", vec![n_idx, arg_name_lit]);
                call_children.push(n_arg);
            }
            // Pass recursive results for each recursive arg.
            for (arg_name, arg_type) in &cons.args {
                if arg_type == type_name {
                    let arg_name_lit = push_leaf(out, "lit", Value::from(arg_name.clone()));
                    let n_arg = push_node(out, "field-access", vec![n_idx, arg_name_lit]);
                    // self(self)(c0, ..., c_{N-1})(n.arg)
                    let rec_full = push_node(out, "call", vec![rec_app, n_arg]);
                    call_children.push(rec_full);
                }
            }
            push_node(out, "call", call_children)
        };

        // if n.tag == "Ci" then then_branch else else_branch
        else_branch = push_node(out, "if", vec![tag_eq, then_branch, else_branch]);
    }

    else_branch
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
        "data" => desugar_data(original, ast, out),
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

        // Should produce a `desugar-error` node, not a `lit` with
        // `__data_error__` prefix (Bug 0.4).
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "desugar-error");
        let reason = root.payload.get("reason").and_then(|v| v.as_str()).unwrap_or("");
        let kind = root.payload.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        assert_eq!(kind, "data-error");
        assert!(
            reason.contains("strict positivity"),
            "expected strict positivity error, got: {}",
            reason
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

        // Last statement: Nat.rec = rec { ... }
        // (Bug 0.3: recursor is wrapped in a rec region for self-reference)
        let rec_assign = out.get(AstRef::new(children[3].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(rec_assign.kind, "assign");
        let rec_children = rec_assign.payload.get("children").unwrap().as_array().unwrap();
        // Target: var("Nat.rec")
        let target = out.get(AstRef::new(rec_children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(target.kind, "var");
        assert_eq!(target.payload, json!("Nat.rec"));
        // Value: rec region (containing the self-application bindings)
        let value = out.get(AstRef::new(rec_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(value.kind, "rec");
    }

    // -----------------------------------------------------------------------
    // Bug 0.1: Constructor json node references arg variables, not string lits
    // -----------------------------------------------------------------------

    #[test]
    fn data_constructor_references_arg_var_node() {
        // Desugaring `data Nat = Zero | Succ(Nat)` should produce a Succ
        // constructor whose body is a `json` template that references the
        // `pred` *var node* (not a string literal "pred").
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

        // Statement 2: Succ = (pred) -> json{...}
        let succ_assign = out.get(AstRef::new(children[2].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(succ_assign.kind, "assign");
        let succ_children = succ_assign.payload.get("children").unwrap().as_array().unwrap();
        // Value: function (rich payload: params + body)
        let succ_fn = out.get(AstRef::new(succ_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(succ_fn.kind, "function");
        // params: ["pred"]
        let params = succ_fn.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], json!("pred"));
        // body: json node with children [template, pred_var]
        let body_idx = succ_fn.payload.get("body").unwrap().as_u64().unwrap() as usize;
        let body = out.get(AstRef::new(body_idx)).unwrap();
        assert_eq!(body.kind, "json");
        let body_children = body.payload.get("children").unwrap().as_array().unwrap();
        // Child 0: template json node (string payload)
        let template = out.get(AstRef::new(body_children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(template.kind, "json");
        let template_str = template.payload.as_str().expect("template must be a string");
        assert!(template_str.contains("\"pred\""), "template must contain pred field: {}", template_str);
        assert!(template_str.contains("$0"), "template must contain $0 placeholder: {}", template_str);
        // Child 1: the arg var node (pred), NOT a string literal
        let arg_node = out.get(AstRef::new(body_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(arg_node.kind, "var", "arg must be a var node, not a lit");
        assert_eq!(arg_node.payload, json!("pred"));
    }

    // -----------------------------------------------------------------------
    // Bug 0.2: Recursor if nodes are binary-nested (3 children each)
    // -----------------------------------------------------------------------

    #[test]
    fn data_recursor_if_nodes_are_binary_nested() {
        // The recursor dispatch should use nested 3-child `if` nodes,
        // not a flat 5-child `if`.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node("Nat", &[("Zero", &[]), ("Succ", &[("pred", "Nat")])]));
        let out = apply_desugar(&lang, &ast);

        // Walk the AST and verify every `if` node has exactly 3 children.
        for node in out.nodes() {
            if node.kind == "if" {
                let children = node.payload.get("children").unwrap().as_array().unwrap();
                assert_eq!(
                    children.len(),
                    3,
                    "if node must have exactly 3 children, got {}: payload={}",
                    children.len(),
                    node.payload
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // ADR-0082 Phase 1: AST-based constructor extraction (parser-produced
    // `data` nodes with `alt` tree children)
    // -----------------------------------------------------------------------

    /// Build a full AST arena mimicking the parser output for
    /// `data Nat = Zero | Succ(Nat)`.
    fn parser_style_ast_nat() -> Ast {
        let mut ast = Ast::new();
        // var("Zero") — index 0
        let zero = ast.push(AstNode::new("var").with_payload(json!("Zero")));
        // var("Succ") — index 1
        let succ = ast.push(AstNode::new("var").with_payload(json!("Succ")));
        // var("Nat") — index 2
        let nat = ast.push(AstNode::new("var").with_payload(json!("Nat")));
        // call(Succ, Nat) — index 3
        let call = ast.push(
            AstNode::new("call").with_payload(json!({ "children": [succ, nat] })),
        );
        // alt(Zero, call) — index 4
        let alt = ast.push(
            AstNode::new("alt").with_payload(json!({ "children": [zero, call] })),
        );
        // data(Nat, alt) — index 5
        let _data = ast.push(
            AstNode::new("data").with_payload(json!({ "name": "Nat", "children": [alt] })),
        );
        ast
    }

    #[test]
    fn data_ast_based_extraction_nat() {
        // Build a parser-style AST for `data Nat = Zero | Succ(Nat)` and
        // verify the desugar pass extracts constructors from the alt tree.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let ast = parser_style_ast_nat();
        let out = apply_desugar(&lang, &ast);

        // The root should be a `block` with 4 statements.
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 4, "expected 4 statements (pred + 2 cons + rec)");

        // Each statement should be an `assign`.
        for (i, child_idx) in children.iter().enumerate() {
            let node = out.get(AstRef::new(child_idx.as_u64().unwrap() as usize)).unwrap();
            assert_eq!(node.kind, "assign", "statement {} should be assign, got {}", i, node.kind);
        }

        // Verify the predicate target is var("Nat").
        let pred_assign = out.get(AstRef::new(children[0].as_u64().unwrap() as usize)).unwrap();
        let pred_children = pred_assign.payload.get("children").unwrap().as_array().unwrap();
        let target = out.get(AstRef::new(pred_children[0].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(target.kind, "var");
        assert_eq!(target.payload, json!("Nat"));

        // Verify the Succ constructor has arg name "nat" (lowercased type).
        let succ_assign = out.get(AstRef::new(children[2].as_u64().unwrap() as usize)).unwrap();
        let succ_children = succ_assign.payload.get("children").unwrap().as_array().unwrap();
        let succ_fn = out.get(AstRef::new(succ_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(succ_fn.kind, "function");
        let params = succ_fn.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], json!("nat"), "arg name should be lowercased type name");
    }

    #[test]
    fn data_ast_based_extraction_bool() {
        // Build a parser-style AST for `data Bool = True | False`.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        let true_var = ast.push(AstNode::new("var").with_payload(json!("True")));
        let false_var = ast.push(AstNode::new("var").with_payload(json!("False")));
        let alt = ast.push(
            AstNode::new("alt").with_payload(json!({ "children": [true_var, false_var] })),
        );
        ast.push(
            AstNode::new("data").with_payload(json!({ "name": "Bool", "children": [alt] })),
        );

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 4, "expected 4 statements (pred + 2 cons + rec)");
    }

    #[test]
    fn data_ast_based_extraction_single_constructor() {
        // A data declaration with a single constructor (no alt tree).
        // `data Unit = Unit` → child is a var node, not an alt.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        let unit_var = ast.push(AstNode::new("var").with_payload(json!("Unit")));
        ast.push(
            AstNode::new("data").with_payload(json!({ "name": "Unit", "children": [unit_var] })),
        );

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // 1 predicate + 1 constructor + 1 recursor = 3
        assert_eq!(children.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Backward compatibility: hand-built data nodes still work
    // -----------------------------------------------------------------------

    #[test]
    fn data_handbuilt_backward_compat() {
        // Verify that hand-built data nodes (with payload.constructors)
        // still desugar correctly after the AST-based extraction change.
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
        assert_eq!(children.len(), 4);

        // Verify the Succ constructor uses "pred" (from hand-built payload).
        let succ_assign = out.get(AstRef::new(children[2].as_u64().unwrap() as usize)).unwrap();
        let succ_children = succ_assign.payload.get("children").unwrap().as_array().unwrap();
        let succ_fn = out.get(AstRef::new(succ_children[1].as_u64().unwrap() as usize)).unwrap();
        let params = succ_fn.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(params[0], json!("pred"), "hand-built arg name should be 'pred'");
    }

    // -----------------------------------------------------------------------
    // ADR-0082 Phase 1: type-annot arg extraction (explicit arg names)
    // -----------------------------------------------------------------------

    #[test]
    fn data_ast_based_extraction_type_annot_args() {
        // `data Maybe = Nothing | Just(value : Nat)` — the `Just` constructor
        // has an explicit arg name via a `type-annot` node. The desugar pass
        // should extract arg name "value" and type "Nat" from the type-annot
        // node's children [subject(var), annot(var)].
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        // var("Nothing") — index 0
        let nothing = ast.push(AstNode::new("var").with_payload(json!("Nothing")));
        // var("Just") — index 1
        let just = ast.push(AstNode::new("var").with_payload(json!("Just")));
        // var("value") — index 2
        let value = ast.push(AstNode::new("var").with_payload(json!("value")));
        // var("Nat") — index 3
        let nat = ast.push(AstNode::new("var").with_payload(json!("Nat")));
        // type-annot(value, Nat) — index 4
        let type_annot = ast.push(
            AstNode::new("type-annot").with_payload(json!({ "children": [value, nat] })),
        );
        // call(Just, type-annot) — index 5
        let call = ast.push(
            AstNode::new("call").with_payload(json!({ "children": [just, type_annot] })),
        );
        // alt(Nothing, call) — index 6
        let alt = ast.push(
            AstNode::new("alt").with_payload(json!({ "children": [nothing, call] })),
        );
        // data(Maybe, alt) — index 7
        ast.push(
            AstNode::new("data").with_payload(json!({ "name": "Maybe", "children": [alt] })),
        );

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 4, "expected 4 statements (pred + 2 cons + rec)");

        // The Just constructor (statement 2) should have param "value".
        let just_assign = out.get(AstRef::new(children[2].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(just_assign.kind, "assign");
        let just_children = just_assign.payload.get("children").unwrap().as_array().unwrap();
        let just_fn = out.get(AstRef::new(just_children[1].as_u64().unwrap() as usize)).unwrap();
        assert_eq!(just_fn.kind, "function");
        let params = just_fn.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(
            params[0], json!("value"),
            "type-annot arg name should be 'value', not a placeholder"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 2.1: Recursor handles N constructors (not just 2)
    // -----------------------------------------------------------------------

    /// Count nodes of a given kind in the output AST.
    fn count_kind(out: &Ast, kind: &str) -> usize {
        out.nodes().iter().filter(|n| n.kind == kind).count()
    }

    /// Extract the recursor's middle-function param count from a desugared
    /// `data` block. The recursor is the last `assign` in the block; its
    /// value is a `rec` region whose first binding (`__rec_inner_<T>`) holds
    /// the outer function `(self) -> (c0, ..., c_{N-1}) -> (n) -> ...`. We
    /// walk down: assign → rec → assign[0] → function → body(function) →
    /// function (middle) → params.len().
    fn recursor_param_count(out: &Ast, block_children: &[Value]) -> Option<usize> {
        // Last child is the recursor assign.
        let rec_assign_idx = block_children.last()?.as_u64()? as usize;
        let rec_assign = out.get(AstRef::new(rec_assign_idx))?;
        let rec_children = rec_assign.payload.get("children")?.as_array()?;
        // rec_assign value (child 1) is the `rec` region.
        let rec_region_idx = rec_children.get(1)?.as_u64()? as usize;
        let rec_region = out.get(AstRef::new(rec_region_idx))?;
        assert_eq!(rec_region.kind, "rec");
        let rec_region_children = rec_region.payload.get("children")?.as_array()?;
        // First binding: __rec_inner_<T> = <outer function>
        let inner_assign_idx = rec_region_children.first()?.as_u64()? as usize;
        let inner_assign = out.get(AstRef::new(inner_assign_idx))?;
        let inner_children = inner_assign.payload.get("children")?.as_array()?;
        // Value (child 1) is the outer function: (self) -> mid_fn
        let outer_fn_idx = inner_children.get(1)?.as_u64()? as usize;
        let outer_fn = out.get(AstRef::new(outer_fn_idx))?;
        assert_eq!(outer_fn.kind, "function");
        // Body is the middle function: (c0, ..., c_{N-1}) -> inner_fn
        let mid_fn_idx = outer_fn.payload.get("body")?.as_u64()? as usize;
        let mid_fn = out.get(AstRef::new(mid_fn_idx))?;
        assert_eq!(mid_fn.kind, "function");
        let params = mid_fn.payload.get("params")?.as_array()?;
        Some(params.len())
    }

    #[test]
    fn data_rgb_three_nullary_constructors() {
        // data RGB = Red | Green | Blue — 3 nullary constructors.
        // The recursor must dispatch on all 3, not just the first 2.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "RGB",
            &[("Red", &[]), ("Green", &[]), ("Blue", &[])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // 1 predicate + 3 constructors + 1 recursor = 5
        assert_eq!(
            children.len(),
            5,
            "expected 5 statements (pred + 3 cons + rec)"
        );

        // The recursor middle function should have 3 params (c0, c1, c2).
        let nparams = recursor_param_count(&out, children).expect("recursor params");
        assert_eq!(nparams, 3, "RGB recursor should take 3 constructor params");

        // The dispatch should produce 3 `if` nodes (one per constructor).
        let if_count = count_kind(&out, "if");
        assert_eq!(
            if_count, 3,
            "RGB recursor dispatch should have 3 if nodes, got {}",
            if_count
        );

        // Every `if` node must have exactly 3 children.
        for node in out.nodes() {
            if node.kind == "if" {
                let kids = node.payload.get("children").unwrap().as_array().unwrap();
                assert_eq!(kids.len(), 3, "if node must have 3 children");
            }
        }
    }

    #[test]
    fn data_nat3_three_constructors_with_recursion() {
        // data Nat3 = Zero | Succ1(Nat3) | Succ2(Nat3) — 3 constructors,
        // two of which are recursive. The recursor must handle all 3.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "Nat3",
            &[
                ("Zero", &[]),
                ("Succ1", &[("pred", "Nat3")]),
                ("Succ2", &[("pred", "Nat3")]),
            ],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // 1 predicate + 3 constructors + 1 recursor = 5
        assert_eq!(children.len(), 5);

        // Recursor should take 3 params.
        let nparams = recursor_param_count(&out, children).expect("recursor params");
        assert_eq!(nparams, 3, "Nat3 recursor should take 3 constructor params");

        // 3 if nodes in the dispatch.
        let if_count = count_kind(&out, "if");
        assert_eq!(if_count, 3, "Nat3 recursor dispatch should have 3 if nodes");

        // Every if node has 3 children.
        for node in out.nodes() {
            if node.kind == "if" {
                let kids = node.payload.get("children").unwrap().as_array().unwrap();
                assert_eq!(kids.len(), 3);
            }
        }
    }

    #[test]
    fn data_tree_binary_recursion() {
        // data Tree = Leaf | Node(Tree, Tree) — a constructor with TWO
        // recursive args. The step call must pass both recursive results.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "Tree",
            &[("Leaf", &[]), ("Node", &[("left", "Tree"), ("right", "Tree")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        // 1 predicate + 2 constructors + 1 recursor = 4
        assert_eq!(children.len(), 4);

        // Recursor should take 2 params (c0=Leaf, c1=Node).
        let nparams = recursor_param_count(&out, children).expect("recursor params");
        assert_eq!(nparams, 2, "Tree recursor should take 2 constructor params");

        // 2 if nodes in the dispatch.
        let if_count = count_kind(&out, "if");
        assert_eq!(if_count, 2, "Tree recursor dispatch should have 2 if nodes");

        // The Node branch's step call should have:
        //   c1 (callee) + 2 args (n.left, n.right) + 2 recursive results = 5 children.
        // Find the `call` node whose first child is a var named "c1" and
        // which has 5 children (the Node step call).
        let mut found_node_step = false;
        for node in out.nodes() {
            if node.kind != "call" {
                continue;
            }
            let kids = match node.payload.get("children").and_then(|c| c.as_array()) {
                Some(a) => a,
                None => continue,
            };
            if kids.len() < 2 {
                continue;
            }
            // First child should be a var "c1".
            let callee_idx = kids[0].as_u64().map(|v| v as usize);
            if let Some(ci) = callee_idx {
                if let Some(callee) = out.get(AstRef::new(ci)) {
                    if callee.kind == "var" && callee.payload == json!("c1") {
                        // This is the Node step call. It should have 5 children:
                        // [c1, n.left, n.right, rec(left), rec(right)]
                        assert_eq!(
                            kids.len(),
                            5,
                            "Node step call should have 5 children (callee + 2 args + 2 rec results), got {}: payload={}",
                            kids.len(),
                            node.payload
                        );
                        found_node_step = true;
                    }
                }
            }
        }
        assert!(
            found_node_step,
            "did not find the Node step call with 5 children"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 2.2: Strict positivity check walks type-expression AST
    // -----------------------------------------------------------------------

    #[test]
    fn data_positivity_nested_function_type_rejected() {
        // data Bad2 = Cons((Bad2 -> Int) -> Int) — Bad2 occurs in a
        // negative position inside a nested function domain.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "Bad2",
            &[("Cons", &[("f", "(Bad2 -> Int) -> Int")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "desugar-error");
        let reason = root.payload.get("reason").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            reason.contains("strict positivity"),
            "expected strict positivity error for nested function type, got: {}",
            reason
        );
    }

    #[test]
    fn data_positivity_positive_codomain_accepted() {
        // data Ok = Cons(Int -> Ok) — Ok only appears in the positive
        // (codomain) position, so this is strictly positive.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node("Ok", &[("Cons", &[("f", "Int -> Ok")])]));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "block",
            "Int -> Ok should be accepted (positive codomain), got kind: {}",
            root.kind
        );
    }

    #[test]
    fn data_positivity_bare_type_accepted() {
        // data Ok2 = Cons(Ok) — a bare type reference is always positive.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node("Ok2", &[("Cons", &[("v", "Ok2")])]));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "block",
            "bare type Ok2 should be accepted (always positive), got kind: {}",
            root.kind
        );
    }

    #[test]
    fn data_positivity_negative_inside_parameterized_rejected() {
        // data Bad3 = Cons(Wrapper(Bad3 -> Int)) — Bad3 occurs in a
        // negative position inside a parameterized type's argument.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "Bad3",
            &[("Cons", &[("w", "Wrapper(Bad3 -> Int)")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "desugar-error");
        let reason = root
            .payload
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            reason.contains("strict positivity"),
            "expected strict positivity error for negative inside Wrapper, got: {}",
            reason
        );
    }

    #[test]
    fn data_positivity_parameterized_positive_accepted() {
        // data Good = Cons(List(Good)) — Good appears inside a
        // parameterized type's argument, which is a positive position.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "Good",
            &[("Cons", &[("xs", "List(Good)")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "block",
            "List(Good) should be accepted (positive inside parameterized), got kind: {}",
            root.kind
        );
    }
}
