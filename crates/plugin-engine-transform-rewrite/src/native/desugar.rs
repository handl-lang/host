//! Native desugar rewrites for the ADR-0074 v0.1 sugar forms.
//!
//! Moved verbatim from `handl-runtime::desugar_pass` (ADR-0094 Phase 3): the
//! functions below take the [`Language`], the original (pre-remap) node, the
//! full original [`Ast`], the output arena, and the old→new `remap` table —
//! the [`NativeRewriteRegistry`] built by [`desugar_native_registry`] adapts
//! each to the [`RewriteCtx`] API.
//!
//! Dispatch stays data-driven: [`DESUGAR_NATIVES`] maps node kinds to native
//! names, and the engine consults it only when no declarative
//! [`TransformDef`](handl_language::TransformDef) exists for the kind —
//! mirroring the legacy `try_rust_desugar` fallback that ran after the
//! declarative `AstShapeDef::desugar` dispatch.

use handl_language::{Ast, AstNode, AstRef, LangError, Language};
use handl_plugin_engine_transform::native::{NativeRewriteRegistry, RewriteCtx};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

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
fn remap_children_from(original: &AstNode, remap: &[Option<usize>], start: usize) -> Vec<usize> {
    let children = original.payload.get("children").and_then(|c| c.as_array());
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
    out.push(AstNode::new(kind).with_payload(payload)).index()
}

/// Push a node with children into the output arena and return its index.
fn push_node(out: &mut Ast, kind: &str, children: Vec<usize>) -> usize {
    let payload = if children.is_empty() {
        Value::Null
    } else {
        serde_json::json!({ "children": children })
    };
    out.push(AstNode::new(kind).with_payload(payload)).index()
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

/// Push a `function` node with termination metadata in its payload
/// (ADR-0082 step 3).
///
/// The `termination` field declares that the function is structurally
/// recursive on the parameter at `decreasing_arg`, decreasing via the
/// field named `decreasing_field`. The typechecker reads this metadata
/// to recognize structural recursion without hardcoded name matching.
fn push_function_with_termination(
    out: &mut Ast,
    params: &[&str],
    body_idx: usize,
    decreasing_arg: usize,
    decreasing_field: &str,
) -> usize {
    let params_arr: Vec<Value> = params.iter().map(|s| Value::from(*s)).collect();
    let payload = serde_json::json!({
        "params": params_arr,
        "body": body_idx,
        "termination": {
            "decreasing_arg": decreasing_arg,
            "decreasing_field": decreasing_field
        }
    });
    out.push(AstNode::new("function").with_payload(payload))
        .index()
}

/// Find the first recursive field name across all constructors of a type.
///
/// A recursive field is a constructor argument whose type expression is the
/// type being defined, either bare (`Nat`) or applied (`Vec(n - 1, a)`).
fn find_first_recursive_field(constructors: &[DataConstructor], type_name: &str) -> Option<String> {
    for cons in constructors {
        for (arg_name, arg_type) in &cons.args {
            let mut parser = TypeExprParser::new(arg_type);
            if let Some(expr) = parser.parse() {
                match &expr {
                    TypeExpr::Atom(name) | TypeExpr::App(name, _) if name == type_name => {
                        return Some(arg_name.clone());
                    }
                    _ => {}
                }
            }
        }
    }
    None
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
#[allow(dead_code)] // exercised only by the direct unit tests (opt-in, unregistered)
fn desugar_if(original: &AstNode, ast: &Ast, out: &mut Ast, remap: &[Option<usize>]) -> AstRef {
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
fn desugar_for(original: &AstNode, ast: &Ast, out: &mut Ast, remap: &[Option<usize>]) -> AstRef {
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
struct DataParam {
    name: String,
    typ: String,
}

#[derive(Debug, Clone)]
struct DataConstructor {
    name: String,
    args: Vec<(String, String)>, // (arg_name, arg_type_string)
    /// For each type parameter, an optional result-index expression string.
    /// Used by the predicate to check that the value was built with the
    /// requested indices (e.g. `Nil` in `Vec` satisfies `n == 0`).
    result_indices: Vec<Option<String>>,
}

/// Parse a `data` node's payload into the type name, optional type parameters,
/// and constructor list.
///
/// Supports three payload shapes:
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
/// **Parser-produced (ADR-0082 Phase 6.3):** two children:
/// - child 0 is the type expression (`var Name` or `call(Name, params...)`)
/// - child 1 is the constructor list (`alt` tree or single constructor).
///
/// **Legacy parser-produced:** `payload.name` + one child (constructor list).
fn parse_data_payload(
    lang: &Language,
    node: &AstNode,
    ast: &Ast,
) -> Option<(String, Vec<DataParam>, Vec<DataConstructor>)> {
    // Backward compatibility: if `payload.constructors` exists (hand-built
    // test nodes), use the old structured-JSON path.
    if let Some(constructors) = node.payload.get("constructors").and_then(|c| c.as_array()) {
        let name = node
            .payload
            .get("name")
            .and_then(|v| v.as_str())?
            .to_string();
        let cons = constructors
            .iter()
            .map(|c| {
                let cname = c
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = c
                    .get("args")
                    .and_then(|a| a.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|a| {
                                let aname = a
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("_")
                                    .to_string();
                                let atype = a
                                    .get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                (aname, atype)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let result_indices = c
                    .get("result_indices")
                    .and_then(|r| r.as_array())
                    .map(|arr| arr.iter().map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                DataConstructor {
                    name: cname,
                    args,
                    result_indices,
                }
            })
            .collect();
        return Some((name, Vec::new(), cons));
    }

    let children = node.payload.get("children").and_then(|c| c.as_array())?;
    let (name, params) = if children.len() >= 2 {
        // New parser form: child 0 = type expression with optional indices.
        let type_idx = children[0].as_u64()? as usize;
        let type_node = ast.get(AstRef::new(type_idx))?;
        parse_type_expr(lang, type_node, ast)?
    } else {
        // Legacy form: name in payload, no parameters.
        (
            node.payload
                .get("name")
                .and_then(|v| v.as_str())?
                .to_string(),
            Vec::new(),
        )
    };

    // The constructor list is the last child for both new and legacy forms.
    let cons_idx = children.last()?.as_u64()? as usize;
    let cons_node = ast.get(AstRef::new(cons_idx))?;
    let mut constructors = Vec::new();
    collect_constructors(lang, cons_node, ast, &mut constructors, &params);
    Some((name, params, constructors))
}

/// Parse a type expression node into the type name and its parameters.
///
/// - `var("Name")` -> ("Name", [])
/// - `call(var("Name"), type-annot(n, Nat), type-annot(a, Type), ...)`
///      -> ("Name", [("n","Nat"), ("a","Type"), ...])
fn parse_type_expr(lang: &Language, node: &AstNode, ast: &Ast) -> Option<(String, Vec<DataParam>)> {
    match node.kind.as_str() {
        "var" => {
            let name = node.payload.as_str()?.to_string();
            Some((name, Vec::new()))
        }
        "call" => {
            let children = node.payload.get("children").and_then(|c| c.as_array())?;
            let callee_idx = children.first()?.as_u64()? as usize;
            let callee = ast.get(AstRef::new(callee_idx))?;
            let name = callee.payload.as_str()?.to_string();
            let mut params = Vec::new();
            for param_val in children.iter().skip(1) {
                let param_idx = param_val.as_u64()? as usize;
                let param_node = ast.get(AstRef::new(param_idx))?;
                if let Some((name, typ)) = parse_type_param(lang, param_node, ast) {
                    params.push(DataParam { name, typ });
                }
            }
            Some((name, params))
        }
        _ => None,
    }
}

/// Parse a single type parameter node, which is either a bare `var` (name
/// only, type unknown) or a `type-annot(var(name), type)`.
fn parse_type_param(lang: &Language, node: &AstNode, ast: &Ast) -> Option<(String, String)> {
    let has_subject = lang
        .child_role_index(&node.kind, "subject")
        .ok()
        .flatten()
        .is_some();
    let has_annot = lang
        .child_role_index(&node.kind, "annot")
        .ok()
        .flatten()
        .is_some();
    if has_subject && has_annot {
        // `type-annot(subject, annot)`
        let children = node.payload.get("children").and_then(|c| c.as_array())?;
        let name_idx = children.first()?.as_u64()? as usize;
        let name_node = ast.get(AstRef::new(name_idx))?;
        let name = name_node.payload.as_str()?.to_string();
        let type_idx = children.get(1)?.as_u64()? as usize;
        let type_node = ast.get(AstRef::new(type_idx))?;
        let typ = node_to_type_string(ast, type_node);
        Some((name, typ))
    } else if let Some(name) = node.payload.as_str() {
        // bare `var`
        Some((name.to_string(), "_".to_string()))
    } else {
        None
    }
}

/// Recursively collect constructors from an `alt` tree or a single
/// constructor node.
///
/// A constructor may be wrapped in a `type-annot` that gives its result type
/// (e.g. `Nil : Vec(0, a)`). The result type's arguments become the
/// constructor's `result_indices`.
fn collect_constructors(
    lang: &Language,
    node: &AstNode,
    ast: &Ast,
    out: &mut Vec<DataConstructor>,
    params: &[DataParam],
) {
    match node.kind.as_str() {
        "alt" => {
            let children = node.payload.get("children").and_then(|c| c.as_array());
            if let Some(arr) = children {
                for child_val in arr {
                    if let Some(idx) = child_val.as_u64() {
                        if let Some(child) = ast.get(AstRef::new(idx as usize)) {
                            collect_constructors(lang, child, ast, out, params);
                        }
                    }
                }
            }
        }
        "type-annot" => {
            let children = node.payload.get("children").and_then(|c| c.as_array());
            if let Some(ta_arr) = children {
                if ta_arr.len() >= 2 {
                    let cons_idx = ta_arr[0].as_u64().map(|v| v as usize);
                    let result_idx = ta_arr[1].as_u64().map(|v| v as usize);
                    let result_indices = result_idx
                        .and_then(|idx| ast.get(AstRef::new(idx)))
                        .map(|n| extract_result_indices_from_type(n, ast, params.len()))
                        .unwrap_or_else(|| vec![None; params.len()]);
                    if let Some(cons_idx) = cons_idx {
                        if let Some(cons_node) = ast.get(AstRef::new(cons_idx)) {
                            collect_constructor_leaf(
                                lang,
                                cons_node,
                                ast,
                                out,
                                params,
                                result_indices,
                            );
                        }
                    }
                }
            }
        }
        _ => {
            collect_constructor_leaf(lang, node, ast, out, params, vec![None; params.len()]);
        }
    }
}

/// Extract a list of result-index strings from a type expression node.
///
/// - `var("Name")` with `expected` params -> empty (all `None`).
/// - `call(var("Name"), e0, e1, ...)` -> `Some(node_to_type_string(ei))` for
///   each argument, padded with `None` up to `expected`.
fn extract_result_indices_from_type(
    node: &AstNode,
    ast: &Ast,
    expected: usize,
) -> Vec<Option<String>> {
    match node.kind.as_str() {
        "call" => {
            let children = node.payload.get("children").and_then(|c| c.as_array());
            if let Some(arr) = children {
                let mut out: Vec<Option<String>> = arr
                    .iter()
                    .skip(1)
                    .map(|v| {
                        v.as_u64()
                            .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                            .map(|n| node_to_type_string(ast, n))
                            .filter(|s| !s.is_empty())
                            .map(Some)
                            .unwrap_or(None)
                    })
                    .collect();
                while out.len() < expected {
                    out.push(None);
                }
                out.truncate(expected);
                return out;
            }
        }
        _ => {}
    }
    vec![None; expected]
}

/// Collect a single constructor leaf (`var` or `call`), optionally with
/// pre-computed result indices.
fn collect_constructor_leaf(
    lang: &Language,
    node: &AstNode,
    ast: &Ast,
    out: &mut Vec<DataConstructor>,
    params: &[DataParam],
    mut result_indices: Vec<Option<String>>,
) {
    match node.kind.as_str() {
        "var" => {
            let cname = node.payload.as_str().unwrap_or("").to_string();
            // Bootstrap: nullary constructors with no explicit result type get
            // `0` for the first `Nat` parameter (the canonical base index).
            if result_indices.iter().all(|r| r.is_none()) {
                if let Some(i) = params.iter().position(|p| p.typ == "Nat") {
                    result_indices[i] = Some("0".to_string());
                }
            }
            out.push(DataConstructor {
                name: cname,
                args: Vec::new(),
                result_indices,
            });
        }
        "call" => {
            let children = node.payload.get("children").and_then(|c| c.as_array());
            let arr = match children {
                Some(a) => a,
                None => return,
            };
            let callee_idx = arr.first().and_then(|v| v.as_u64()).map(|v| v as usize);
            let cname = callee_idx
                .and_then(|idx| ast.get(AstRef::new(idx)))
                .and_then(|n| n.payload.as_str())
                .unwrap_or("")
                .to_string();
            let mut args = Vec::new();
            for (i, arg_val) in arr.iter().enumerate().skip(1) {
                if let Some(arg_idx) = arg_val.as_u64() {
                    if let Some(arg_node) = ast.get(AstRef::new(arg_idx as usize)) {
                        let (arg_name, arg_type) = parse_constructor_arg(lang, ast, arg_node, i);
                        args.push((arg_name, arg_type));
                    }
                }
            }
            out.push(DataConstructor {
                name: cname,
                args,
                result_indices,
            });
        }
        _ => {}
    }
}

/// Parse a single constructor argument node into (arg_name, arg_type_string).
fn parse_constructor_arg(
    lang: &Language,
    ast: &Ast,
    arg_node: &AstNode,
    position: usize,
) -> (String, String) {
    if lang
        .child_role_index(&arg_node.kind, "annot")
        .ok()
        .flatten()
        .is_some()
    {
        let ta_children = arg_node.payload.get("children").and_then(|c| c.as_array());
        if let Some(ta_arr) = ta_children {
            if ta_arr.len() >= 2 {
                let subject_idx = ta_arr[0].as_u64().map(|v| v as usize);
                let annot_idx = ta_arr[1].as_u64().map(|v| v as usize);
                let name = subject_idx
                    .and_then(|idx| ast.get(AstRef::new(idx)))
                    .and_then(|n| n.payload.as_str())
                    .unwrap_or("_")
                    .to_string();
                let typ = annot_idx
                    .and_then(|idx| ast.get(AstRef::new(idx)))
                    .map(|n| node_to_type_string(ast, n))
                    .unwrap_or_else(|| "_".to_string());
                return (name, typ);
            }
        }
    }
    let typ = node_to_type_string(ast, arg_node);
    let name = if typ.is_empty() {
        format!("_{}", position - 1)
    } else {
        derive_arg_name_from_type(&typ)
    };
    (name, typ)
}

/// Derive a default argument name from a type string by lowercasing the head
/// identifier.
fn derive_arg_name_from_type(type_str: &str) -> String {
    type_str
        .split(|c: char| c == '(' || c == ' ' || c == '.' || c == '-' || c == '+')
        .next()
        .unwrap_or("_")
        .to_lowercase()
}

/// Reconstruct a source-like string for a type/index expression AST node.
fn node_to_type_string(ast: &Ast, node: &AstNode) -> String {
    match node.kind.as_str() {
        "var" => node.payload.as_str().unwrap_or("").to_string(),
        "value" => node.payload.to_string(),
        "lit" => node.payload.to_string(),
        "call" => {
            let children = match node.payload.get("children").and_then(|c| c.as_array()) {
                Some(a) => a,
                None => return String::new(),
            };
            let callee = children
                .first()
                .and_then(|v| v.as_u64())
                .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                .map(|n| node_to_type_string(ast, n))
                .unwrap_or_default();
            let args: Vec<String> = children
                .iter()
                .skip(1)
                .filter_map(|v| {
                    v.as_u64()
                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                        .map(|n| node_to_type_string(ast, n))
                })
                .collect();
            format!("{}({})", callee, args.join(", "))
        }
        "type-annot" => {
            let children = node.payload.get("children").and_then(|c| c.as_array());
            if let Some(arr) = children {
                if arr.len() >= 2 {
                    let left = arr[0]
                        .as_u64()
                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                        .map(|n| node_to_type_string(ast, n))
                        .unwrap_or_default();
                    let right = arr[1]
                        .as_u64()
                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                        .map(|n| node_to_type_string(ast, n))
                        .unwrap_or_default();
                    return format!("{} : {}", left, right);
                }
            }
            String::new()
        }
        "field-access" => {
            let children = node.payload.get("children").and_then(|c| c.as_array());
            if let Some(arr) = children {
                if arr.len() >= 2 {
                    let left = arr[0]
                        .as_u64()
                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                        .map(|n| node_to_type_string(ast, n))
                        .unwrap_or_default();
                    let right = arr[1]
                        .as_u64()
                        .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                        .map(|n| node_to_type_string(ast, n))
                        .unwrap_or_default();
                    return format!("{}.{}", left, right);
                }
            }
            String::new()
        }
        kind if kind.starts_with("binary.") => {
            let children = match node.payload.get("children").and_then(|c| c.as_array()) {
                Some(a) => a,
                None => return String::new(),
            };
            if children.len() >= 2 {
                let op = kind.strip_prefix("binary.").unwrap_or("");
                let op_symbol = match op {
                    "add" => "+",
                    "sub" => "-",
                    "mul" => "*",
                    "div" => "/",
                    "mod" => "%",
                    _ => op,
                };
                let left = children[0]
                    .as_u64()
                    .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                    .map(|n| node_to_type_string(ast, n))
                    .unwrap_or_default();
                let right = children[1]
                    .as_u64()
                    .and_then(|idx| ast.get(AstRef::new(idx as usize)))
                    .map(|n| node_to_type_string(ast, n))
                    .unwrap_or_default();
                return format!("{} {} {}", left, op_symbol, right);
            }
            String::new()
        }
        _ => String::new(),
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
fn check_strict_positivity(
    type_name: &str,
    constructors: &[DataConstructor],
) -> Result<(), String> {
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
    /// A bare atom (identifier or number), e.g. `Nat` or `0`.
    Atom(String),
    /// A parameterized type application, e.g. `Vec(n - 1, a)`.
    App(String, Vec<TypeExpr>),
    /// A function type `A -> B`.
    Func(Box<TypeExpr>, Box<TypeExpr>),
    /// An arithmetic operator, e.g. `n - 1`.
    Op(String, Box<TypeExpr>, Box<TypeExpr>),
}

/// Tokenize a type-expression string into `->`, `(`, `)`, `,`, `+`, `-`,
/// `*`, `/`, `%`, numbers, and identifiers.
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
        } else if c.is_ascii_digit() {
            let mut num = String::new();
            while let Some(&c2) = chars.peek() {
                if c2.is_ascii_digit() {
                    num.push(c2);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(num);
        } else if c == '+' || c == '-' || c == '*' || c == '/' || c == '%' {
            tokens.push(c.to_string());
            chars.next();
        } else if c.is_alphabetic() || c == '_' {
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

/// A simple recursive-descent parser for type expressions that also handles
/// arithmetic index expressions like `n - 1` in `Vec(n - 1, a)`.
///
/// Grammar:
/// ```text
/// TypeExpr     = FuncType
/// FuncType     = AddType ('->' FuncType)?
/// AddType      = MulType (('+' | '-') MulType)*
/// MulType      = AppType (('*' | '/' | '%') AppType)*
/// AppType      = AtomType ('(' TypeExprList ')')?
/// AtomType     = identifier | number | '(' FuncType ')'
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
        let left = self.parse_add_type()?;
        if self.peek() == Some("->") {
            self.advance(); // consume `->`
            let right = self.parse_func_type()?;
            Some(TypeExpr::Func(Box::new(left), Box::new(right)))
        } else {
            Some(left)
        }
    }

    fn parse_add_type(&mut self) -> Option<TypeExpr> {
        let mut left = self.parse_mul_type()?;
        while let Some(op) = self.peek() {
            if op == "+" || op == "-" {
                let op = op.to_string();
                self.advance();
                let right = self.parse_mul_type()?;
                left = TypeExpr::Op(op, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Some(left)
    }

    fn parse_mul_type(&mut self) -> Option<TypeExpr> {
        let mut left = self.parse_app_type()?;
        while let Some(op) = self.peek() {
            if op == "*" || op == "/" || op == "%" {
                let op = op.to_string();
                self.advance();
                let right = self.parse_app_type()?;
                left = TypeExpr::Op(op, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Some(left)
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
            Some(tok) if tok.chars().all(|c| c.is_ascii_digit()) => {
                self.advance();
                Some(TypeExpr::Atom(tok.to_string()))
            }
            Some(tok)
                if tok != "->"
                    && tok != ")"
                    && tok != ","
                    && tok != "+"
                    && tok != "-"
                    && tok != "*"
                    && tok != "/"
                    && tok != "%" =>
            {
                self.advance();
                Some(TypeExpr::Atom(tok.to_string()))
            }
            _ => None,
        }
    }
}

/// Convert a `TypeExpr` into an AST node index in the output arena.
///
/// Used to embed index expressions (e.g. `n - 1`, `0`, `a`) inside the
/// generated predicate and recursor bodies.
fn type_expr_to_ast(out: &mut Ast, expr: &TypeExpr) -> usize {
    match expr {
        TypeExpr::Atom(s) => {
            if let Ok(n) = s.parse::<i64>() {
                push_leaf(out, "value", Value::from(n))
            } else if s.parse::<f64>().is_ok() {
                push_leaf(out, "value", Value::from(s.as_str()))
            } else {
                push_leaf(out, "var", Value::from(s.clone()))
            }
        }
        TypeExpr::App(name, args) => {
            let callee = push_leaf(out, "var", Value::from(name.clone()));
            let mut children = vec![callee];
            for arg in args {
                children.push(type_expr_to_ast(out, arg));
            }
            push_node(out, "call", children)
        }
        TypeExpr::Func(domain, codomain) => {
            // Function types do not occur as index expressions; build an
            // application of `->` as a last-resort placeholder.
            let arrow = push_leaf(out, "var", Value::from("->"));
            let lhs = type_expr_to_ast(out, domain);
            let rhs = type_expr_to_ast(out, codomain);
            push_node(out, "call", vec![arrow, lhs, rhs])
        }
        TypeExpr::Op(op, lhs, rhs) => {
            let kind = match op.as_str() {
                "+" => "binary.add",
                "-" => "binary.sub",
                "*" => "binary.mul",
                "/" => "binary.div",
                "%" => "binary.mod",
                _ => "binary.add",
            };
            let left = type_expr_to_ast(out, lhs);
            let right = type_expr_to_ast(out, rhs);
            push_node(out, kind, vec![left, right])
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
/// - `Op(op, l, r)`: recurse into both sides (arithmetic is positive).
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
        TypeExpr::Op(_, l, r) => {
            check_strict_positivity_rec(l, type_name)?;
            check_strict_positivity_rec(r, type_name)
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
        TypeExpr::Op(_, l, r) => occurs_in(l, type_name) || occurs_in(r, type_name),
    }
}

/// Desugar a `data` declaration into a `block` of definitions:
/// the predicate, each constructor, and the recursor (ADR-0082 §1,
/// extended for Phase 6.3 inductive families).
///
/// `data Vec(n : Nat, a : Type) = Nil | Cons(a, Vec(n - 1, a))` desugars
/// to a `block` containing:
/// - `Vec = (n, a, v) -> (v.tag == "Nil" and n == 0) or
///     (v.tag == "Cons" and Vec(n - 1, a)(v.xs))`
/// - `Nil = { tag: "Nil" }`
/// - `Cons = (x, xs) -> { tag: "Cons", x: x, xs: xs }`
/// - `Vec.rec = (motive, base, step) -> (n, a, v) -> ...`
fn desugar_data(lang: &Language, original: &AstNode, ast: &Ast, out: &mut Ast) -> Option<AstRef> {
    let (type_name, params, constructors) = parse_data_payload(lang, original, ast)?;

    // Step 2: strict positivity check.
    if let Err(reason) = check_strict_positivity(&type_name, &constructors) {
        let err_payload = serde_json::json!({
            "kind": "data-error",
            "reason": reason
        });
        let err_idx = push_leaf(out, "desugar-error", err_payload);
        return Some(AstRef::new(err_idx));
    }

    let mut stmts: Vec<usize> = Vec::new();

    // 1. Predicate: Type = (params..., v) -> <checks>
    let mut pred_param_refs: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
    pred_param_refs.push("v");
    let v_var = push_leaf(out, "var", Value::from("v"));
    let self_var = push_leaf(out, "var", Value::from("self"));
    let pred_body = build_predicate_body(out, &type_name, &params, &constructors, v_var, self_var);
    let inner_fn = push_function(out, &pred_param_refs, pred_body);
    let outer_fn = push_function(out, &["self"], inner_fn);
    let self_app = push_node(out, "call", vec![outer_fn, outer_fn]);
    let mut pred_call_children: Vec<usize> = vec![self_app];
    for p in &pred_param_refs {
        pred_call_children.push(push_leaf(out, "var", Value::from(*p)));
    }
    let pred_body_call = push_node(out, "call", pred_call_children);
    let pred_fn = push_function(out, &pred_param_refs, pred_body_call);
    let pred_target = push_leaf(out, "var", Value::from(type_name.clone()));
    let pred_assign = push_node(out, "assign", vec![pred_target, pred_fn]);
    stmts.push(pred_assign);

    // 2. Constructors.
    for cons in &constructors {
        let cons_target = push_leaf(out, "var", Value::from(cons.name.clone()));
        let cons_value = if cons.args.is_empty() {
            push_leaf(out, "lit", serde_json::json!({ "tag": cons.name }))
        } else {
            let arg_vars: Vec<usize> = cons
                .args
                .iter()
                .map(|(name, _)| push_leaf(out, "var", Value::from(name.clone())))
                .collect();
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
            push_function(out, &param_names, body_idx)
        };
        let cons_assign = push_node(out, "assign", vec![cons_target, cons_value]);
        stmts.push(cons_assign);
    }

    // 3. Recursor with optional motive parameter:
    //    Type.rec = (motive, c0, ..., c_{N-1}) -> (params..., v) -> dispatch
    let rec_name = format!("{}.rec", type_name);
    let rec_target = push_leaf(out, "var", Value::from(rec_name.clone()));
    let rec_inner_fn = build_recursor(out, &type_name, &params, &constructors);

    let inner_name = format!("__rec_inner_{}", type_name);
    let inner_target = push_leaf(out, "var", Value::from(inner_name.clone()));
    let inner_assign = push_node(out, "assign", vec![inner_target, rec_inner_fn]);

    let inner_var1 = push_leaf(out, "var", Value::from(inner_name.clone()));
    let inner_var2 = push_leaf(out, "var", Value::from(inner_name));
    let self_app = push_node(out, "call", vec![inner_var1, inner_var2]);
    let rec_inner_target = push_leaf(out, "var", Value::from(rec_name));
    let rec_inner_assign = push_node(out, "assign", vec![rec_inner_target, self_app]);

    let rec_region = push_node(out, "rec", vec![inner_assign, rec_inner_assign]);
    let rec_assign = push_node(out, "assign", vec![rec_target, rec_region]);
    stmts.push(rec_assign);

    let block = push_node(out, "block", stmts);
    Some(AstRef::new(block))
}

/// Build the predicate dispatch body for the inner function `(params..., v) -> ...`.
///
/// The predicate must be able to call itself recursively, but it is built before
/// the top-level `Type` binding is available in the environment. We use the
/// `self` parameter of the enclosing fixed-point function: `self(self)` yields
/// the same inner predicate, which can be applied to the recursive argument.
///
/// The dispatch is an `if` chain so that only the matching constructor's branch
/// is evaluated (the `binary.or`/`binary.and` primitives are not short-circuiting).
fn build_predicate_body(
    out: &mut Ast,
    type_name: &str,
    params: &[DataParam],
    constructors: &[DataConstructor],
    v_idx: usize,
    self_idx: usize,
) -> usize {
    if constructors.is_empty() {
        return push_leaf(out, "lit", Value::Bool(false));
    }

    // self(self) gives the recursive predicate closure.
    let rec_app = push_node(out, "call", vec![self_idx, self_idx]);

    let mut else_branch = push_leaf(out, "lit", Value::Bool(false));
    for cons in constructors.iter().rev() {
        let condition = build_constructor_condition(out, params, cons, v_idx);
        let then_branch =
            build_constructor_predicate_check(out, type_name, params, cons, v_idx, rec_app);
        else_branch = push_node(out, "if", vec![condition, then_branch, else_branch]);
    }
    else_branch
}

/// Build the guard for a constructor: `v.tag == "ConsName"` plus any
/// non-tautological result-index equalities. These checks are safe to evaluate
/// eagerly because they do not recursively inspect the value.
fn build_constructor_condition(
    out: &mut Ast,
    params: &[DataParam],
    cons: &DataConstructor,
    v_idx: usize,
) -> usize {
    let tag_lit = push_leaf(out, "lit", Value::from("tag"));
    let tag_access = push_node(out, "field-access", vec![v_idx, tag_lit]);
    let cons_name_lit = push_leaf(out, "lit", Value::from(cons.name.clone()));
    let mut condition = push_node(out, "binary.eq", vec![tag_access, cons_name_lit]);

    for (i, param) in params.iter().enumerate() {
        if let Some(index_expr) = cons.result_indices.get(i).and_then(|r| r.as_ref()) {
            let mut parser = TypeExprParser::new(index_expr);
            if let Some(idx_expr) = parser.parse() {
                let idx_ast = type_expr_to_ast(out, &idx_expr);
                // Skip tautological checks like `n == n`.
                if let Some(idx_node) = out.get(AstRef::new(idx_ast)) {
                    if idx_node.kind == "var"
                        && idx_node.payload.as_str() == Some(param.name.as_str())
                    {
                        continue;
                    }
                }
                let param_var = push_leaf(out, "var", Value::from(param.name.clone()));
                let eq = push_node(out, "binary.eq", vec![param_var, idx_ast]);
                condition = push_node(out, "binary.and", vec![condition, eq]);
            }
        }
    }

    // For parameters typed as `Nat`, ensure the requested index is
    // non-negative. This keeps well-founded indexed families such as `Fin`
    // from accepting values whose recursive index has dropped below zero
    // (e.g. `Fin(0, FS(FZ))` requires `Fin(-1, FZ)`, which must be empty).
    for param in params {
        if param.typ == "Nat" {
            let param_var = push_leaf(out, "var", Value::from(param.name.clone()));
            let zero = push_leaf(out, "value", Value::from(0));
            let gte = push_node(out, "binary.gte", vec![param_var, zero]);
            condition = push_node(out, "binary.and", vec![condition, gte]);
        }
    }

    condition
}

/// Build the then-branch of a constructor's predicate check: a conjunction of
/// recursive predicate calls, one for each recursive constructor argument.
/// Because this is used inside the `then` branch of an `if`, the tag is known
/// to match, so field accesses are safe.
fn build_constructor_predicate_check(
    out: &mut Ast,
    type_name: &str,
    _params: &[DataParam],
    cons: &DataConstructor,
    v_idx: usize,
    rec_app: usize,
) -> usize {
    let mut expr = push_leaf(out, "lit", Value::Bool(true));
    let mut had_recursive = false;

    for (arg_name, arg_type) in &cons.args {
        let mut parser = TypeExprParser::new(arg_type);
        if let Some(type_expr) = parser.parse() {
            let is_recursive = match &type_expr {
                TypeExpr::Atom(name) => name == type_name,
                TypeExpr::App(name, _) => name == type_name,
                _ => false,
            };
            if is_recursive {
                had_recursive = true;
                let arg_lit = push_leaf(out, "lit", Value::from(arg_name.clone()));
                let field_access = push_node(out, "field-access", vec![v_idx, arg_lit]);

                let call = match type_expr {
                    TypeExpr::App(_, index_exprs) => {
                        let mut call_children = vec![rec_app];
                        for idx_expr in &index_exprs {
                            call_children.push(type_expr_to_ast(out, idx_expr));
                        }
                        call_children.push(field_access);
                        push_node(out, "call", call_children)
                    }
                    _ => push_node(out, "call", vec![rec_app, field_access]),
                };
                expr = push_node(out, "binary.and", vec![expr, call]);
            }
        }
    }

    if had_recursive {
        expr
    } else {
        push_leaf(out, "lit", Value::Bool(true))
    }
}

/// Build the recursor inner function (self-application form):
///   (self) -> (motive, c0, ..., c_{N-1}) -> (params..., v) -> dispatch
///
/// `motive` is the first method argument; it is carried through recursive
/// calls but is not used by the runtime dispatch. The desugar pass fills it
/// with a user-supplied return motive for `match` expressions, or `null`
/// for bare recursor calls.
fn build_recursor(
    out: &mut Ast,
    type_name: &str,
    params: &[DataParam],
    constructors: &[DataConstructor],
) -> usize {
    let self_var = push_leaf(out, "var", Value::from("self"));
    let motive_var = push_leaf(out, "var", Value::from("motive"));

    let param_names: Vec<String> = (0..constructors.len()).map(|i| format!("c{}", i)).collect();
    let param_vars: Vec<usize> = param_names
        .iter()
        .map(|name| push_leaf(out, "var", Value::from(name.clone())))
        .collect();

    let mut inner_param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
    inner_param_names.push("v".to_string());
    let v_var = push_leaf(out, "var", Value::from("v"));

    let dispatch = build_recursor_dispatch(
        out,
        type_name,
        params,
        constructors,
        v_var,
        motive_var,
        &param_vars,
        self_var,
    );

    let inner_param_refs: Vec<&str> = inner_param_names.iter().map(|s| s.as_str()).collect();
    let inner_fn = match find_first_recursive_field(constructors, type_name) {
        Some(field) => {
            let decreasing_arg = params.len();
            push_function_with_termination(out, &inner_param_refs, dispatch, decreasing_arg, &field)
        }
        None => push_function(out, &inner_param_refs, dispatch),
    };

    let mut mid_param_names = vec!["motive"];
    mid_param_names.extend(param_names.iter().map(|s| s.as_str()));
    let mid_fn = push_function(out, &mid_param_names, inner_fn);
    push_function(out, &["self"], mid_fn)
}

/// Build the recursor dispatch.
fn build_recursor_dispatch(
    out: &mut Ast,
    type_name: &str,
    _params: &[DataParam],
    constructors: &[DataConstructor],
    v_idx: usize,
    motive_var: usize,
    param_vars: &[usize],
    self_idx: usize,
) -> usize {
    if constructors.is_empty() {
        return push_leaf(out, "lit", Value::Null);
    }

    let self_self = push_node(out, "call", vec![self_idx, self_idx]);
    let mut rec_app_children = vec![self_self, motive_var];
    rec_app_children.extend(param_vars.iter().copied());
    let rec_app = push_node(out, "call", rec_app_children);

    let mut else_branch = push_leaf(out, "lit", Value::Null);

    for i in (0..constructors.len()).rev() {
        let cons = &constructors[i];

        let tag_lit = push_leaf(out, "lit", Value::from("tag"));
        let n_tag = push_node(out, "field-access", vec![v_idx, tag_lit]);
        let cons_name_lit = push_leaf(out, "lit", Value::from(cons.name.clone()));
        let tag_eq = push_node(out, "binary.eq", vec![n_tag, cons_name_lit]);

        let then_branch = if cons.args.is_empty() {
            param_vars[i]
        } else {
            let mut call_children = vec![param_vars[i]];
            // Pass all constructor args.
            for (arg_name, _) in &cons.args {
                let arg_name_lit = push_leaf(out, "lit", Value::from(arg_name.clone()));
                let n_arg = push_node(out, "field-access", vec![v_idx, arg_name_lit]);
                call_children.push(n_arg);
            }
            // Pass recursive results for each recursive arg.
            for (arg_name, arg_type) in &cons.args {
                let mut parser = TypeExprParser::new(arg_type);
                if let Some(type_expr) = parser.parse() {
                    let is_recursive = match &type_expr {
                        TypeExpr::Atom(name) => name == type_name,
                        TypeExpr::App(name, _) => name == type_name,
                        _ => false,
                    };
                    if is_recursive {
                        let arg_name_lit = push_leaf(out, "lit", Value::from(arg_name.clone()));
                        let n_arg = push_node(out, "field-access", vec![v_idx, arg_name_lit]);

                        let rec_full = match type_expr {
                            TypeExpr::App(_, index_exprs) => {
                                let mut rec_children = vec![rec_app];
                                for idx_expr in &index_exprs {
                                    rec_children.push(type_expr_to_ast(out, idx_expr));
                                }
                                rec_children.push(n_arg);
                                push_node(out, "call", rec_children)
                            }
                            _ => push_node(out, "call", vec![rec_app, n_arg]),
                        };
                        call_children.push(rec_full);
                    }
                }
            }
            push_node(out, "call", call_children)
        };

        else_branch = push_node(out, "if", vec![tag_eq, then_branch, else_branch]);
    }

    else_branch
}

// ---------------------------------------------------------------------------
// 9. `match n { Zero => 0, Succ(pred) => 1 }` → `Nat.rec(0, (pred, rec) => 1)(n)`
//     (ADR-0082 step 4: match as sugar over the recursor)
// ---------------------------------------------------------------------------

/// Parsed match arm: constructor name, arg names from the pattern, and the
/// remapped body index in the output arena.
struct MatchArm {
    cons_name: String,
    arg_names: Vec<String>,
    body_idx: usize,
}

/// Parse a match arm's pattern to determine if it's a constructor pattern.
///
/// Returns `Some(MatchArm)` if the pattern is constructor-like:
/// - `var` node (not `_`): nullary constructor. Name = payload, no args.
/// - `call` node (child 0 is `var`): N-ary constructor. Name = child 0's
///   payload, arg names from children 1+ (each must be a `var`).
///
/// Returns `None` if the pattern is NOT constructor-like:
/// - `var("_")`: wildcard (conditional match, not inductive).
/// - `lit`, `object`, `array`, etc.: non-constructor pattern.
/// - `call` with non-`var` callee: not a constructor pattern.
fn parse_arm_pattern(out: &Ast, pattern: &AstNode) -> Option<MatchArm> {
    match pattern.kind.as_str() {
        "var" => {
            let name = pattern.payload.as_str()?;
            if name == "_" {
                return None; // wildcard — conditional match
            }
            Some(MatchArm {
                cons_name: name.to_string(),
                arg_names: Vec::new(),
                body_idx: 0, // filled by caller
            })
        }
        "call" => {
            let call_children = pattern.payload.get("children").and_then(|c| c.as_array())?;
            if call_children.is_empty() {
                return None;
            }
            let callee_idx = call_children[0].as_u64()? as usize;
            let callee = out.get(AstRef::new(callee_idx))?;
            if callee.kind != "var" {
                return None;
            }
            let cons_name = callee.payload.as_str()?.to_string();
            let mut arg_names = Vec::new();
            for arg_val in &call_children[1..] {
                let arg_idx = arg_val.as_u64()? as usize;
                let arg_node = out.get(AstRef::new(arg_idx))?;
                if arg_node.kind != "var" {
                    return None;
                }
                arg_names.push(arg_node.payload.as_str()?.to_string());
            }
            Some(MatchArm {
                cons_name,
                arg_names,
                body_idx: 0,
            })
        }
        _ => None,
    }
}

/// Desugar an inductive `match` expression into a recursor application
/// (ADR-0082 step 4 and step 6, Phase 6.3 optional motives).
///
/// Non-indexed example:
/// `match n { Zero: 0, Succ(pred): 1 }` becomes
/// `Nat.rec(null, 0, (pred, __rec_0) ~> 1)(n)`.
///
/// Indexed-family example:
/// `match (v : Vec(n, a)) { VNil: 0, VCons(x, xs): __rec_0 + 1 }` becomes
/// `Vec.rec(null, 0, step)(n, a, v)`.
///
/// A motive is passed as the first recursor argument:
/// `match v return (i) ~> T with { ... }` becomes
/// `Type.rec(motive, ...)(indices..., v)`.
///
/// **Guards:**
/// - Only fires when ALL arms have constructor-like patterns (var nodes that
///   aren't `_`, or call nodes with a var callee). If any arm uses a wildcard
///   (`_`) or non-constructor pattern, the match is left unchanged.
/// - For indexed families, the scrutinee must be a (possibly parenthesised)
///   type-annotated call to the matching family (`v : Type(indices...)`).
///   Otherwise the match is left for the runtime `match` primitive.
fn desugar_match_to_recursor(
    lang: &Language,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Option<AstRef> {
    let children = original
        .payload
        .get("children")
        .and_then(|c| c.as_array())?;
    if children.len() < 2 {
        return None;
    }

    let has_motive = children.len() >= 3;
    let scrutinee = remap_child(original, remap, 0)?;
    let motive_idx = if has_motive {
        Some(remap_child(original, remap, 1)?)
    } else {
        None
    };
    let block_idx = remap_child(original, remap, if has_motive { 2 } else { 1 })?;

    let block_node = out.get(AstRef::new(block_idx))?;
    let arm_indices: Vec<usize> = block_node
        .payload
        .get("children")
        .and_then(|c| c.as_array())?
        .iter()
        .map(|v| v.as_u64().map(|i| i as usize))
        .collect::<Option<_>>()?;

    if arm_indices.is_empty() {
        return None;
    }

    let mut arms: Vec<MatchArm> = Vec::new();
    for &arm_idx in &arm_indices {
        let arm_node = out.get(AstRef::new(arm_idx))?;
        let arm_children = arm_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())?;
        if arm_children.len() < 2 {
            return None;
        }
        let pattern_idx = arm_children[0].as_u64()? as usize;
        let body_idx = arm_children[1].as_u64()? as usize;
        let pattern = out.get(AstRef::new(pattern_idx))?;

        let mut arm = parse_arm_pattern(out, pattern)?;
        arm.body_idx = body_idx;
        arms.push(arm);
    }

    let mut cons_to_type: std::collections::HashMap<
        String,
        (String, Vec<DataParam>, Vec<DataConstructor>),
    > = std::collections::HashMap::new();
    for node in ast.nodes() {
        if node.kind == "data" {
            if let Some((type_name, params, constructors)) = parse_data_payload(lang, node, ast) {
                for cons in &constructors {
                    cons_to_type.insert(
                        cons.name.clone(),
                        (type_name.clone(), params.clone(), constructors.clone()),
                    );
                }
            }
        }
    }

    let first_cons_name = &arms[0].cons_name;
    let (type_name, params, constructors) = cons_to_type.get(first_cons_name)?;

    // For indexed families, the scrutinee must be a (possibly parenthesised)
    // type-annotated expression `v : Type(indices...)`. The annotation gives us
    // the index arguments for the recursor application; the subject is the
    // value being matched. Non-indexed families keep the existing behaviour.
    let (value_node, index_nodes): (usize, Vec<usize>) = if params.is_empty() {
        (scrutinee, Vec::new())
    } else {
        let mut current = scrutinee;

        // Unwrap a single-child `paren` if present.
        // Data-driven: any node whose shape declares a `body` role and has
        // exactly one child is treated as a transparent wrapper.
        if let Some(paren) = out.get(AstRef::new(current)) {
            let is_paren_like = lang
                .child_role_index(&paren.kind, "body")
                .ok()
                .flatten()
                .is_some();
            let paren_children = paren.payload.get("children").and_then(|c| c.as_array())?;
            if is_paren_like && paren_children.len() == 1 {
                current = paren_children.first()?.as_u64()? as usize;
            }
        }

        let (value_idx, annot_idx) = {
            let annotated = out.get(AstRef::new(current))?;
            // Data-driven: a type-annotation shape declares `subject` and
            // `annot` child roles. Any shape with both roles is treated as a
            // type annotation (ADR-0081).
            let subject_role = lang
                .child_role_index(&annotated.kind, "subject")
                .ok()
                .flatten();
            let annot_role = lang
                .child_role_index(&annotated.kind, "annot")
                .ok()
                .flatten();
            let (Some(subject_role), Some(annot_role)) = (subject_role, annot_role) else {
                return None;
            };
            let children = annotated
                .payload
                .get("children")
                .and_then(|c| c.as_array())?;
            let value_idx = children.get(subject_role)?.as_u64()? as usize;
            let annot_idx = children.get(annot_role)?.as_u64()? as usize;
            (value_idx, annot_idx)
        };

        let annot = out.get(AstRef::new(annot_idx))?;
        if annot.kind != "call" {
            return None;
        }
        let annot_children = annot.payload.get("children").and_then(|c| c.as_array())?;
        let callee_idx = annot_children.first()?.as_u64()? as usize;
        let callee = out.get(AstRef::new(callee_idx))?;
        if callee.kind != "var" || callee.payload.as_str()? != type_name.as_str() {
            return None;
        }

        let mut index_nodes = Vec::with_capacity(params.len());
        for val in annot_children.iter().skip(1) {
            index_nodes.push(val.as_u64()? as usize);
        }
        if index_nodes.len() != params.len() {
            return None;
        }
        (value_idx, index_nodes)
    };

    if arms.len() != constructors.len() {
        return None;
    }

    let mut arm_map: std::collections::HashMap<&str, &MatchArm> = std::collections::HashMap::new();
    for arm in &arms {
        if !constructors.iter().any(|c| c.name == arm.cons_name) {
            return None;
        }
        if arm_map.insert(arm.cons_name.as_str(), arm).is_some() {
            return None;
        }
    }

    let rec_name = format!("{}.rec", type_name);
    let rec_var = push_leaf(out, "var", Value::from(rec_name));

    let mut rec_args: Vec<usize> = Vec::new();
    // Optional motive as the first recursor argument.
    rec_args.push(motive_idx.unwrap_or_else(|| push_leaf(out, "lit", Value::Null)));
    for cons in constructors {
        let arm = arm_map.get(cons.name.as_str())?;
        if cons.args.is_empty() {
            rec_args.push(arm.body_idx);
        } else {
            let mut step_params: Vec<String> = arm.arg_names.clone();
            for (_arg_name, arg_type) in &cons.args {
                let mut parser = TypeExprParser::new(arg_type);
                let is_recursive = parser
                    .parse()
                    .map(|expr| match &expr {
                        TypeExpr::Atom(name) | TypeExpr::App(name, _) => name == type_name,
                        _ => false,
                    })
                    .unwrap_or(false);
                if is_recursive {
                    step_params.push(format!("__rec_{}", step_params.len() - arm.arg_names.len()));
                }
            }
            let param_refs: Vec<&str> = step_params.iter().map(|s| s.as_str()).collect();
            let step_fn = push_function(out, &param_refs, arm.body_idx);
            rec_args.push(step_fn);
        }
    }

    let mut call_children = vec![rec_var];
    call_children.extend(rec_args);
    let rec_call = push_node(out, "call", call_children);

    let mut final_children = vec![rec_call];
    final_children.extend(index_nodes);
    final_children.push(value_node);
    let full_call = push_node(out, "call", final_children);

    Some(AstRef::new(full_call))
}

// ---------------------------------------------------------------------------
// Rust desugar dispatch (registry-based, ADR-0081 engine genericity)
// ---------------------------------------------------------------------------

/// Type of a Rust-level desugar handler registered in [`desugar_registry`].
///
/// Each handler takes the `Language`, the original (pre-remap) node, the
/// full original AST, the output arena, and the old→new remap table. It
/// returns the replacement node's `AstRef` in the output arena, or `None`
/// if the handler decides not to desugar this particular node (e.g. the
/// `while` guard that skips the state-passing form).
type DesugarFn = fn(&Language, &AstNode, &Ast, &mut Ast, &[Option<usize>]) -> Option<AstRef>;

// --- Adapter wrappers ---------------------------------------------------
//
// These thin wrappers adapt the individual desugar functions (which have
// heterogeneous signatures — some take `lang`, some return `AstRef` vs
// `Option<AstRef>`) to the uniform `DesugarFn` type expected by the
// registry.

fn desugar_nullish_wrapped(
    _lang: &Language,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Option<AstRef> {
    Some(desugar_nullish(original, ast, out, remap))
}

fn desugar_opt_chain_wrapped(
    _lang: &Language,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Option<AstRef> {
    Some(desugar_opt_chain(original, ast, out, remap))
}

fn desugar_while_wrapped(
    _lang: &Language,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Option<AstRef> {
    desugar_while(original, ast, out, remap)
}

fn desugar_do_while_wrapped(
    _lang: &Language,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Option<AstRef> {
    Some(desugar_do_while(original, ast, out, remap))
}

fn desugar_for_wrapped(
    _lang: &Language,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    remap: &[Option<usize>],
) -> Option<AstRef> {
    Some(desugar_for(original, ast, out, remap))
}

fn desugar_data_wrapped(
    lang: &Language,
    original: &AstNode,
    ast: &Ast,
    out: &mut Ast,
    _remap: &[Option<usize>],
) -> Option<AstRef> {
    desugar_data(lang, original, ast, out)
}



/// Build (once) and return the static desugar function table — a table
/// mapping node kind strings to handler functions.
///
/// This is a data-driven dispatch: the registry is a `HashMap` table, not
/// a hardcoded `match` statement. New sugar forms can be added by
/// inserting a new entry here (or, in the future, by loading entries from
/// language data). The handlers themselves still know about the kinds they
/// generate — this is inherent to a code generator — but the *dispatch* is
/// table-driven and extensible.
///
/// NOTE: `if` → `match` desugar is implemented as `desugar_if` but NOT
/// registered here. The canonical typechecker has an `if`-specific type
/// rule (`type.if`) that checks the condition is `type.bool` — desugaring
/// `if` to `match` before typechecking would bypass this check and break
/// `typecheck_if_with_non_bool_cond_errors`. The `if` eval scheme
/// (`Seq`/`Choice`/`tag`) already handles `if` correctly at eval time.
/// The `desugar_if` function is tested directly in the test module below.
fn desugar_registry() -> &'static HashMap<&'static str, DesugarFn> {
    static REGISTRY: OnceLock<HashMap<&'static str, DesugarFn>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("nullish", desugar_nullish_wrapped as DesugarFn);
        m.insert("opt-chain", desugar_opt_chain_wrapped as DesugarFn);
        m.insert("while", desugar_while_wrapped as DesugarFn);
        m.insert("do-while", desugar_do_while_wrapped as DesugarFn);
        m.insert("for", desugar_for_wrapped as DesugarFn);
        m.insert("data", desugar_data_wrapped as DesugarFn);
        // ADR-0082 step 4: `match` with constructor patterns desugars to
        // recursor application. Returns `None` (no desugar) for conditional
        // matches (wildcard `_` or non-constructor patterns) — those pass
        // through unchanged and are evaluated by the `match` eval primitive.
        m.insert("match", desugar_match_to_recursor as DesugarFn);
        m
    })
}

/// `(node_kind, native_name)` pairs forming the `"desugar"` pass's native
/// fallback (ADR-0094 §6).
///
/// The engine consults this table for a node only when no declarative
/// [`TransformDef`](handl_language::TransformDef) was found for the node's
/// kind — preserving the legacy ordering where `try_rust_desugar` ran after
/// the declarative `AstShapeDef::desugar` dispatch. `if` is deliberately
/// absent (see [`desugar_registry`]); `match` maps to the
/// constructor-pattern recursor desugar, which declines non-constructor
/// matches via [`RewriteCtx::pass_through`].
pub static DESUGAR_NATIVES: &[(&str, &str)] = &[
    ("nullish", "desugar.nullish"),
    ("opt-chain", "desugar.opt_chain"),
    ("while", "desugar.while"),
    ("do-while", "desugar.do_while"),
    ("for", "desugar.for"),
    ("data", "desugar.data"),
    ("match", "desugar.match_recursor"),
];

/// Adapt a [`DesugarFn`] (the moved legacy signature) into a
/// [`NativeRewriteRegistry`] closure over [`RewriteCtx`].
///
/// A handler returning `None` — e.g. the `while` guard that skips the
/// state-passing form, or `desugar_match_to_recursor` on a conditional
/// match — becomes [`RewriteCtx::pass_through`]: the node is pushed into
/// the output arena unchanged with its children remapped, exactly as the
/// legacy fallthrough did.
fn adapt(f: DesugarFn) -> Box<dyn Fn(&mut RewriteCtx<'_>) -> Result<AstRef, LangError>> {
    Box::new(move |ctx| {
        let lang = ctx.lang();
        let node = ctx.node();
        let ast = ctx.ast();
        let remap = ctx.remap();
        match f(lang, node, ast, ctx.out(), remap) {
            Some(new_ref) => Ok(new_ref),
            None => Ok(ctx.pass_through()),
        }
    })
}

/// Build a [`NativeRewriteRegistry`] wiring every desugar native under its
/// `desugar.<form>` name (ADR-0094 §3 `TransformRule::Native`).
///
/// Registered names: `desugar.nullish`, `desugar.opt_chain`,
/// `desugar.while`, `desugar.do_while`, `desugar.for`, `desugar.data`,
/// `desugar.match_recursor`.
///
/// `desugar.if` is intentionally **not** registered: the canonical
/// typechecker's `type.if` rule must observe `if` nodes, so desugaring
/// `if` → `match` before typechecking would bypass it (see
/// [`desugar_registry`]). `desugar_if` is retained for its direct tests
/// only.
pub fn desugar_native_registry() -> NativeRewriteRegistry {
    let mut reg = NativeRewriteRegistry::new();
    for (kind, name) in DESUGAR_NATIVES {
        let f = *desugar_registry()
            .get(kind)
            .expect("DESUGAR_NATIVES kind must have a handler");
        reg.register(*name, adapt(f));
    }
    reg
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstDesugarChild, AstDesugarCondition, AstDesugarRule, AstShapeDef};
    use serde_json::json;

    /// Shim over the [`RewriteEngine`](crate::RewriteEngine) so the moved
    /// `handl-runtime::desugar_pass` test-suite exercises the generic engine
    /// unchanged — every `apply_desugar` call below is a `"desugar"` pass run
    /// with the native desugar registry wired.
    fn apply_desugar(lang: &Language, ast: &Ast) -> Ast {
        if ast.len() == 0 {
            return Ast::new();
        }
        let engine = crate::RewriteEngine::new().with_desugar_natives();
        let root = AstRef::new(ast.len() - 1);
        engine
            .run_pass(lang, ast, root, "desugar")
            .expect("desugar pass failed")
            .ast
    }

    #[test]
    fn no_desugar_is_identity() {
        let mut lang = Language::new();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
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
        lang.merge_ast_fragment(if_shape).unwrap();
        // Other shapes (no desugar)
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

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
        let cond = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(cond.kind, "lit");
        assert_eq!(cond.payload, json!(true));
        // Second child is the block
        let block = out
            .get(AstRef::new(children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(block.kind, "block");
        let block_children = block.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(block_children.len(), 2);
        // Each arm
        let arm1 = out
            .get(AstRef::new(block_children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(arm1.kind, "arm");
        let arm2 = out
            .get(AstRef::new(block_children[1].as_u64().unwrap() as usize))
            .unwrap();
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
        lang.merge_ast_fragment(if_shape).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

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
        let block = out
            .get(AstRef::new(children[1].as_u64().unwrap() as usize))
            .unwrap();
        let block_children = block.payload.get("children").unwrap().as_array().unwrap();
        let arm2 = out
            .get(AstRef::new(block_children[1].as_u64().unwrap() as usize))
            .unwrap();
        let arm2_children = arm2.payload.get("children").unwrap().as_array().unwrap();
        let else_body = out
            .get(AstRef::new(arm2_children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(
            else_body.kind, "match",
            "nested if should be desugared to match"
        );
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
        lang.merge_ast_fragment(entry_shape).unwrap();
        // Other shapes (no desugar, just so the language is well-formed)
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.array", "array"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.paren", "paren"))
            .unwrap();

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
        let key_node = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(key_node.kind, "paren");
        // The paren's child 0 should be the remapped var("k").
        let paren_children = key_node
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        let inner = out
            .get(AstRef::new(paren_children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(inner.kind, "var");
        assert_eq!(inner.payload, json!("k"));
        // Child 1 should be the remapped value(1).
        let val = out
            .get(AstRef::new(children[1].as_u64().unwrap() as usize))
            .unwrap();
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
        lang.merge_ast_fragment(entry_shape).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();

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
        let key = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(
            key.kind, "var",
            "non-array key should pass through unchanged"
        );
        assert_eq!(key.payload, json!("n"));
    }

    #[test]
    fn keep_rest_preserves_variable_length_tail() {
        // ADR-0060: KeepRest(N) expands to all children from index N
        // onward, each remapped into the output arena.
        let mut lang = Language::new();
        let shape = AstShapeDef::new("ast.call", "call").with_desugar(AstDesugarRule {
            target_kind: "json".to_string(),
            children: vec![AstDesugarChild::Keep(0), AstDesugarChild::KeepRest(1)],
            when: None,
        });
        lang.merge_ast_fragment(shape).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.json", "json"))
            .unwrap();

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
        let template = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(template.kind, "json");
        // Children 1 and 2 are the args
        let arg1 = out
            .get(AstRef::new(children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(arg1.kind, "lit");
        assert_eq!(arg1.payload, json!(42));
        let arg2 = out
            .get(AstRef::new(children[2].as_u64().unwrap() as usize))
            .unwrap();
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
            children: vec![AstDesugarChild::Keep(0), AstDesugarChild::KeepRest(1)],
            when: None,
        });
        lang.merge_ast_fragment(shape).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.json", "json"))
            .unwrap();

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
            block
                .payload
                .get("children")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
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
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
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
            block
                .payload
                .get("children")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
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
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
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
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.binary.gt", "binary.gt"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.assign", "assign"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.while", "while"))
            .unwrap();

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
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.object", "object"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.function", "function"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.while", "while"))
            .unwrap();

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
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.block", "block"))
            .unwrap();
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
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.binary.mul", "binary.mul"))
            .unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.for", "for")).unwrap();

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
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
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
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));
        let out = apply_desugar(&lang, &ast);

        // The root should be a `block` containing the predicate, two
        // constructors, and the recursor (4 assign statements).
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(
            children.len(),
            4,
            "expected 4 statements (pred + 2 cons + rec)"
        );

        // Each statement should be an `assign`.
        for (i, child_idx) in children.iter().enumerate() {
            let node = out
                .get(AstRef::new(child_idx.as_u64().unwrap() as usize))
                .unwrap();
            assert_eq!(
                node.kind, "assign",
                "statement {} should be assign, got {}",
                i, node.kind
            );
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
        let reason = root
            .payload
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let kind = root
            .payload
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");
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
        ast.push(data_node(
            "Maybe",
            &[("None", &[]), ("Some", &[("value", "Int")])],
        ));
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
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();

        // First statement: Nat = (v) -> ...  (assign with var target, function value)
        let pred_assign = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(pred_assign.kind, "assign");
        let pred_children = pred_assign
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        // Target: var("Nat")
        let target = out
            .get(AstRef::new(pred_children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(target.kind, "var");
        assert_eq!(target.payload, json!("Nat"));
        // Value: function
        let value = out
            .get(AstRef::new(pred_children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(value.kind, "function");
    }

    #[test]
    fn data_nat_recursor_is_function() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        let children = root.payload.get("children").unwrap().as_array().unwrap();

        // Last statement: Nat.rec = rec { ... }
        // (Bug 0.3: recursor is wrapped in a rec region for self-reference)
        let rec_assign = out
            .get(AstRef::new(children[3].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(rec_assign.kind, "assign");
        let rec_children = rec_assign
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        // Target: var("Nat.rec")
        let target = out
            .get(AstRef::new(rec_children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(target.kind, "var");
        assert_eq!(target.payload, json!("Nat.rec"));
        // Value: rec region (containing the self-application bindings)
        let value = out
            .get(AstRef::new(rec_children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(value.kind, "rec");
    }

    // -----------------------------------------------------------------------
    // ADR-0082 step 3: Termination metadata on recursor's inner function
    // -----------------------------------------------------------------------

    /// Helper: walk the recursor's function chain to find the inner function
    /// `(n) -> dispatch`. Returns the inner function node.
    fn recursor_inner_fn<'a>(out: &'a Ast, block_children: &[Value]) -> Option<&'a AstNode> {
        let rec_assign_idx = block_children.last()?.as_u64()? as usize;
        let rec_assign = out.get(AstRef::new(rec_assign_idx))?;
        let rec_children = rec_assign.payload.get("children")?.as_array()?;
        let rec_region_idx = rec_children.get(1)?.as_u64()? as usize;
        let rec_region = out.get(AstRef::new(rec_region_idx))?;
        let rec_region_children = rec_region.payload.get("children")?.as_array()?;
        let inner_assign_idx = rec_region_children.first()?.as_u64()? as usize;
        let inner_assign = out.get(AstRef::new(inner_assign_idx))?;
        let inner_children = inner_assign.payload.get("children")?.as_array()?;
        let outer_fn_idx = inner_children.get(1)?.as_u64()? as usize;
        let outer_fn = out.get(AstRef::new(outer_fn_idx))?;
        let mid_fn_idx = outer_fn.payload.get("body")?.as_u64()? as usize;
        let mid_fn = out.get(AstRef::new(mid_fn_idx))?;
        let inner_fn_idx = mid_fn.payload.get("body")?.as_u64()? as usize;
        out.get(AstRef::new(inner_fn_idx))
    }

    #[test]
    fn data_nat_recursor_has_termination_metadata() {
        // ADR-0082 step 3: the recursor's inner function `(n) -> dispatch`
        // should carry a `termination` payload field with decreasing_arg=0
        // and decreasing_field="pred".
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        let inner_fn = recursor_inner_fn(&out, children).expect("inner function");
        assert_eq!(inner_fn.kind, "function");
        let term = inner_fn
            .payload
            .get("termination")
            .expect("termination metadata");
        assert_eq!(term.get("decreasing_arg").unwrap(), &json!(0));
        assert_eq!(term.get("decreasing_field").unwrap(), &json!("pred"));
    }

    #[test]
    fn data_bool_recursor_no_termination_metadata() {
        // Non-recursive types (no recursive constructors) should NOT have
        // termination metadata — the recursor is not structurally recursive.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node("Bool", &[("True", &[]), ("False", &[])]));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        let inner_fn = recursor_inner_fn(&out, children).expect("inner function");
        assert_eq!(inner_fn.kind, "function");
        assert!(
            inner_fn.payload.get("termination").is_none(),
            "non-recursive recursor should not have termination metadata"
        );
    }

    #[test]
    fn data_tree_recursor_termination_uses_first_recursive_field() {
        // data Tree = Leaf | Node(Tree, Tree) — two recursive fields. The
        // termination metadata should use the first recursive field "left".
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        let mut ast = Ast::new();
        ast.push(data_node(
            "Tree",
            &[
                ("Leaf", &[]),
                ("Node", &[("left", "Tree"), ("right", "Tree")]),
            ],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        let inner_fn = recursor_inner_fn(&out, children).expect("inner function");
        let term = inner_fn
            .payload
            .get("termination")
            .expect("termination metadata");
        assert_eq!(term.get("decreasing_arg").unwrap(), &json!(0));
        assert_eq!(term.get("decreasing_field").unwrap(), &json!("left"));
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
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();

        // Statement 2: Succ = (pred) -> json{...}
        let succ_assign = out
            .get(AstRef::new(children[2].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(succ_assign.kind, "assign");
        let succ_children = succ_assign
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        // Value: function (rich payload: params + body)
        let succ_fn = out
            .get(AstRef::new(succ_children[1].as_u64().unwrap() as usize))
            .unwrap();
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
        let template = out
            .get(AstRef::new(body_children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(template.kind, "json");
        let template_str = template
            .payload
            .as_str()
            .expect("template must be a string");
        assert!(
            template_str.contains("\"pred\""),
            "template must contain pred field: {}",
            template_str
        );
        assert!(
            template_str.contains("$0"),
            "template must contain $0 placeholder: {}",
            template_str
        );
        // Child 1: the arg var node (pred), NOT a string literal
        let arg_node = out
            .get(AstRef::new(body_children[1].as_u64().unwrap() as usize))
            .unwrap();
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
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));
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
        let call = ast.push(AstNode::new("call").with_payload(json!({ "children": [succ, nat] })));
        // alt(Zero, call) — index 4
        let alt = ast.push(AstNode::new("alt").with_payload(json!({ "children": [zero, call] })));
        // data(Nat, alt) — index 5
        let _data = ast
            .push(AstNode::new("data").with_payload(json!({ "name": "Nat", "children": [alt] })));
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
        assert_eq!(
            children.len(),
            4,
            "expected 4 statements (pred + 2 cons + rec)"
        );

        // Each statement should be an `assign`.
        for (i, child_idx) in children.iter().enumerate() {
            let node = out
                .get(AstRef::new(child_idx.as_u64().unwrap() as usize))
                .unwrap();
            assert_eq!(
                node.kind, "assign",
                "statement {} should be assign, got {}",
                i, node.kind
            );
        }

        // Verify the predicate target is var("Nat").
        let pred_assign = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        let pred_children = pred_assign
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        let target = out
            .get(AstRef::new(pred_children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(target.kind, "var");
        assert_eq!(target.payload, json!("Nat"));

        // Verify the Succ constructor has arg name "nat" (lowercased type).
        let succ_assign = out
            .get(AstRef::new(children[2].as_u64().unwrap() as usize))
            .unwrap();
        let succ_children = succ_assign
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        let succ_fn = out
            .get(AstRef::new(succ_children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(succ_fn.kind, "function");
        let params = succ_fn.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(
            params[0],
            json!("nat"),
            "arg name should be lowercased type name"
        );
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
        let alt = ast
            .push(AstNode::new("alt").with_payload(json!({ "children": [true_var, false_var] })));
        ast.push(AstNode::new("data").with_payload(json!({ "name": "Bool", "children": [alt] })));

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(
            children.len(),
            4,
            "expected 4 statements (pred + 2 cons + rec)"
        );
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
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 4);

        // Verify the Succ constructor uses "pred" (from hand-built payload).
        let succ_assign = out
            .get(AstRef::new(children[2].as_u64().unwrap() as usize))
            .unwrap();
        let succ_children = succ_assign
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        let succ_fn = out
            .get(AstRef::new(succ_children[1].as_u64().unwrap() as usize))
            .unwrap();
        let params = succ_fn.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(
            params[0],
            json!("pred"),
            "hand-built arg name should be 'pred'"
        );
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
        let type_annot =
            ast.push(AstNode::new("type-annot").with_payload(json!({ "children": [value, nat] })));
        // call(Just, type-annot) — index 5
        let call =
            ast.push(AstNode::new("call").with_payload(json!({ "children": [just, type_annot] })));
        // alt(Nothing, call) — index 6
        let alt =
            ast.push(AstNode::new("alt").with_payload(json!({ "children": [nothing, call] })));
        // data(Maybe, alt) — index 7
        ast.push(AstNode::new("data").with_payload(json!({ "name": "Maybe", "children": [alt] })));

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "block");
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(
            children.len(),
            4,
            "expected 4 statements (pred + 2 cons + rec)"
        );

        // The Just constructor (statement 2) should have param "value".
        let just_assign = out
            .get(AstRef::new(children[2].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(just_assign.kind, "assign");
        let just_children = just_assign
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        let just_fn = out
            .get(AstRef::new(just_children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(just_fn.kind, "function");
        let params = just_fn.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(
            params[0],
            json!("value"),
            "type-annot arg name should be 'value', not a placeholder"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 2.1: Recursor handles N constructors (not just 2)
    // -----------------------------------------------------------------------

    /// Count nodes of a given kind reachable from a starting node.
    fn count_kind_from(out: &Ast, start: usize, kind: &str) -> usize {
        let mut count = 0usize;
        let mut stack = vec![start];
        let mut seen = std::collections::HashSet::new();
        while let Some(idx) = stack.pop() {
            if !seen.insert(idx) {
                continue;
            }
            if let Some(node) = out.get(AstRef::new(idx)) {
                if node.kind == kind {
                    count += 1;
                }
                // Follow both `children` arrays and the `body` pointer.
                if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
                    for c in children {
                        if let Some(i) = c.as_u64() {
                            stack.push(i as usize);
                        }
                    }
                }
                if let Some(body) = node.payload.get("body").and_then(|b| b.as_u64()) {
                    stack.push(body as usize);
                }
            }
        }
        count
    }

    /// Count `if` nodes in the recursor dispatch body.
    fn recursor_dispatch_if_count(out: &Ast, block_children: &[Value]) -> Option<usize> {
        let inner = recursor_inner_fn(out, block_children)?;
        let body = inner.payload.get("body")?.as_u64()? as usize;
        Some(count_kind_from(out, body, "if"))
    }

    /// Extract the recursor's middle-function constructor-param count from a
    /// desugared `data` block. The recursor is the last `assign` in the
    /// block; its value is a `rec` region whose first binding
    /// (`__rec_inner_<T>`) holds the outer function
    /// `(self) -> (motive, c0, ..., c_{N-1}) -> (n) -> ...`. We walk down:
    /// assign → rec → assign[0] → function → body(function) → function
    /// (middle) → params.len() - 1 (the leading `motive` param is not a
    /// constructor argument).
    fn recursor_param_count(out: &Ast, block_children: &[Value]) -> Option<usize> {
        let rec_assign_idx = block_children.last()?.as_u64()? as usize;
        let rec_assign = out.get(AstRef::new(rec_assign_idx))?;
        let rec_children = rec_assign.payload.get("children")?.as_array()?;
        let rec_region_idx = rec_children.get(1)?.as_u64()? as usize;
        let rec_region = out.get(AstRef::new(rec_region_idx))?;
        assert_eq!(rec_region.kind, "rec");
        let rec_region_children = rec_region.payload.get("children")?.as_array()?;
        let inner_assign_idx = rec_region_children.first()?.as_u64()? as usize;
        let inner_assign = out.get(AstRef::new(inner_assign_idx))?;
        let inner_children = inner_assign.payload.get("children")?.as_array()?;
        let outer_fn_idx = inner_children.get(1)?.as_u64()? as usize;
        let outer_fn = out.get(AstRef::new(outer_fn_idx))?;
        assert_eq!(outer_fn.kind, "function");
        let mid_fn_idx = outer_fn.payload.get("body")?.as_u64()? as usize;
        let mid_fn = out.get(AstRef::new(mid_fn_idx))?;
        assert_eq!(mid_fn.kind, "function");
        let params = mid_fn.payload.get("params")?.as_array()?;
        Some(params.len().saturating_sub(1))
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
        let if_count = recursor_dispatch_if_count(&out, children).expect("recursor if count");
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
        let if_count = recursor_dispatch_if_count(&out, children).expect("recursor if count");
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
            &[
                ("Leaf", &[]),
                ("Node", &[("left", "Tree"), ("right", "Tree")]),
            ],
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
        let if_count = recursor_dispatch_if_count(&out, children).expect("recursor if count");
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
        let reason = root
            .payload
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("");
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
        ast.push(data_node("Good", &[("Cons", &[("xs", "List(Good)")])]));
        let out = apply_desugar(&lang, &ast);

        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "block",
            "List(Good) should be accepted (positive inside parameterized), got kind: {}",
            root.kind
        );
    }

    // -----------------------------------------------------------------------
    // ADR-0082 step 4: `match` as sugar over the recursor
    // -----------------------------------------------------------------------

    /// Build a `data` AST node followed by a `match` node for
    /// `match n { Zero => 0, Succ(pred) => 1 }`.
    ///
    /// The match node uses the `children: [scrutinee, block_of_arms]` format
    /// where the block contains `arm` nodes, each with `children: [pattern, body]`.
    fn data_plus_match_nat_ast() -> Ast {
        let mut ast = Ast::new();
        // data Nat = Zero | Succ(Nat)
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));

        // match n { Zero => 0, Succ(pred) => 1 }
        // scrutinee: var("n") — index 1
        let scrut = ast.push(AstNode::new("var").with_payload(json!("n")));
        // arm 1 pattern: var("Zero") — index 2
        let zero_pat = ast.push(AstNode::new("var").with_payload(json!("Zero")));
        // arm 1 body: lit(0) — index 3
        let zero_body = ast.push(AstNode::new("lit").with_payload(json!(0)));
        // arm 1: arm { children: [zero_pat, zero_body] } — index 4
        let arm1 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [zero_pat, zero_body]})));
        // arm 2 pattern: call(var("Succ"), var("pred")) — indices 5, 6, 7
        let succ_var = ast.push(AstNode::new("var").with_payload(json!("Succ")));
        let pred_var = ast.push(AstNode::new("var").with_payload(json!("pred")));
        let succ_pat =
            ast.push(AstNode::new("call").with_payload(json!({"children": [succ_var, pred_var]})));
        // arm 2 body: lit(1) — index 8
        let succ_body = ast.push(AstNode::new("lit").with_payload(json!(1)));
        // arm 2: arm { children: [succ_pat, succ_body] } — index 9
        let arm2 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [succ_pat, succ_body]})));
        // block { arm1, arm2 } — index 10
        let block = ast.push(AstNode::new("block").with_payload(json!({"children": [arm1, arm2]})));
        // match { scrut, block } — index 11
        ast.push(AstNode::new("match").with_payload(json!({"children": [scrut, block]})));
        ast
    }

    #[test]
    fn match_nat_desugars_to_recursor_call() {
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

        let ast = data_plus_match_nat_ast();
        let out = apply_desugar(&lang, &ast);

        // The last node should be a `call` (the recursor application),
        // not a `match`.
        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "call",
            "match should desugar to call (recursor application)"
        );

        // The outer call has 2 children: [rec_call, scrutinee]
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 2);

        // Child 1 (scrutinee) should be var("n")
        let scrut = out
            .get(AstRef::new(children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(scrut.kind, "var");
        assert_eq!(scrut.payload, json!("n"));

        // Child 0 is the inner call: Nat.rec(null, 0, (pred, __rec_0) => 1)
        let rec_call = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(rec_call.kind, "call");
        let rec_children = rec_call
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        // [Nat.rec, motive, base_case, step_fn] = 4 children
        assert_eq!(rec_children.len(), 4);

        // Child 0: var("Nat.rec")
        let rec_var = out
            .get(AstRef::new(rec_children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(rec_var.kind, "var");
        assert_eq!(rec_var.payload, json!("Nat.rec"));

        // Child 1: omitted motive = lit(null)
        let motive = out
            .get(AstRef::new(rec_children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(motive.kind, "lit");
        assert_eq!(motive.payload, json!(null));

        // Child 2: base case = lit(0)
        let base = out
            .get(AstRef::new(rec_children[2].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(base.kind, "lit");
        assert_eq!(base.payload, json!(0));

        // Child 3: step function (pred, __rec_0) => lit(1)
        let step = out
            .get(AstRef::new(rec_children[3].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(step.kind, "function");
        let params = step.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(
            params.len(),
            2,
            "step function should have 2 params (pred, __rec_0)"
        );
        assert_eq!(params[0], json!("pred"));
        assert_eq!(params[1], json!("__rec_0"));
    }

    #[test]
    fn match_with_wildcard_not_desugared() {
        // `match n { Zero => 0, _ => 1 }` — has a wildcard arm, so it
        // should NOT be desugared to a recursor application.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

        let mut ast = Ast::new();
        // data Nat = Zero | Succ(Nat)
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));

        // match n { Zero => 0, _ => 1 }
        let scrut = ast.push(AstNode::new("var").with_payload(json!("n")));
        let zero_pat = ast.push(AstNode::new("var").with_payload(json!("Zero")));
        let zero_body = ast.push(AstNode::new("lit").with_payload(json!(0)));
        let arm1 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [zero_pat, zero_body]})));
        let wildcard = ast.push(AstNode::new("var").with_payload(json!("_")));
        let else_body = ast.push(AstNode::new("lit").with_payload(json!(1)));
        let arm2 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [wildcard, else_body]})));
        let block = ast.push(AstNode::new("block").with_payload(json!({"children": [arm1, arm2]})));
        ast.push(AstNode::new("match").with_payload(json!({"children": [scrut, block]})));

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        // Should remain `match`, not desugared to `call`.
        assert_eq!(root.kind, "match", "wildcard match should not be desugared");
    }

    #[test]
    fn match_conditional_not_desugared() {
        // `match 5 { x: x }` — a conditional match with a binder pattern,
        // not a constructor pattern. Should NOT be desugared.
        let mut lang = Language::new();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.value", "value"))
            .unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

        let mut ast = Ast::new();
        // scrutinee: value(5)
        let scrut = ast.push(AstNode::new("value").with_payload(json!(5)));
        // arm: pattern = var("x"), body = var("x")
        let pat = ast.push(AstNode::new("var").with_payload(json!("x")));
        let body = ast.push(AstNode::new("var").with_payload(json!("x")));
        let arm = ast.push(AstNode::new("arm").with_payload(json!({"children": [pat, body]})));
        let block = ast.push(AstNode::new("block").with_payload(json!({"children": [arm]})));
        ast.push(AstNode::new("match").with_payload(json!({"children": [scrut, block]})));

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "match",
            "conditional match should not be desugared"
        );
    }

    #[test]
    fn match_bool_desugars_to_recursor_call() {
        // data Bool = True | False
        // match b { True => 1, False => 0 } → Bool.rec(1, 0)(b)
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

        let mut ast = Ast::new();
        ast.push(data_node("Bool", &[("True", &[]), ("False", &[])]));

        // match b { True => 1, False => 0 }
        let scrut = ast.push(AstNode::new("var").with_payload(json!("b")));
        let true_pat = ast.push(AstNode::new("var").with_payload(json!("True")));
        let true_body = ast.push(AstNode::new("lit").with_payload(json!(1)));
        let arm1 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [true_pat, true_body]})));
        let false_pat = ast.push(AstNode::new("var").with_payload(json!("False")));
        let false_body = ast.push(AstNode::new("lit").with_payload(json!(0)));
        let arm2 = ast
            .push(AstNode::new("arm").with_payload(json!({"children": [false_pat, false_body]})));
        let block = ast.push(AstNode::new("block").with_payload(json!({"children": [arm1, arm2]})));
        ast.push(AstNode::new("match").with_payload(json!({"children": [scrut, block]})));

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(root.kind, "call", "Bool match should desugar to call");

        // Outer call: [rec_call, scrutinee]
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        assert_eq!(children.len(), 2);

        // Inner call: Bool.rec(null, 1, 0)
        let rec_call = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        let rec_children = rec_call
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        // [Bool.rec, null, 1, 0] = 4 children (motive + 2 nullary constructor args)
        assert_eq!(rec_children.len(), 4);
        let rec_var = out
            .get(AstRef::new(rec_children[0].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(rec_var.payload, json!("Bool.rec"));
        let motive = out
            .get(AstRef::new(rec_children[1].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(motive.payload, json!(null), "default motive is null");
    }

    #[test]
    fn match_arms_out_of_order_still_works() {
        // match n { Succ(pred) => 1, Zero => 0 } — arms in reverse order.
        // The recursor arguments should be in constructor declaration order
        // (Zero first, Succ second), regardless of arm order.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

        let mut ast = Ast::new();
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));

        // match n { Succ(pred) => 1, Zero => 0 }
        let scrut = ast.push(AstNode::new("var").with_payload(json!("n")));
        // arm 1: Succ(pred) => 1
        let succ_var = ast.push(AstNode::new("var").with_payload(json!("Succ")));
        let pred_var = ast.push(AstNode::new("var").with_payload(json!("pred")));
        let succ_pat =
            ast.push(AstNode::new("call").with_payload(json!({"children": [succ_var, pred_var]})));
        let succ_body = ast.push(AstNode::new("lit").with_payload(json!(1)));
        let arm1 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [succ_pat, succ_body]})));
        // arm 2: Zero => 0
        let zero_pat = ast.push(AstNode::new("var").with_payload(json!("Zero")));
        let zero_body = ast.push(AstNode::new("lit").with_payload(json!(0)));
        let arm2 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [zero_pat, zero_body]})));
        let block = ast.push(AstNode::new("block").with_payload(json!({"children": [arm1, arm2]})));
        ast.push(AstNode::new("match").with_payload(json!({"children": [scrut, block]})));

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "call",
            "out-of-order match should desugar to call"
        );

        // Inner call: Nat.rec(null, 0, (pred, __rec_0) => 1)
        let children = root.payload.get("children").unwrap().as_array().unwrap();
        let rec_call = out
            .get(AstRef::new(children[0].as_u64().unwrap() as usize))
            .unwrap();
        let rec_children = rec_call
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(rec_children.len(), 4);

        // Child 1 is the omitted motive; child 2 is Zero's base case.
        let base = out
            .get(AstRef::new(rec_children[2].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(
            base.payload,
            json!(0),
            "Zero base case should be 0 regardless of arm order"
        );

        // Child 3 is Succ's step fn, a function with params [pred, __rec_0].
        let step = out
            .get(AstRef::new(rec_children[3].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(step.kind, "function");
        let params = step.payload.get("params").unwrap().as_array().unwrap();
        assert_eq!(params[0], json!("pred"));
    }

    #[test]
    fn match_incomplete_not_desugared() {
        // match n { Zero => 0 } — only covers one of two constructors.
        // This is not an exhaustive inductive match, so it should NOT desugar.
        let mut lang = Language::new();
        lang.ast
            .merge(AstShapeDef::new("ast.data", "data"))
            .unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

        let mut ast = Ast::new();
        ast.push(data_node(
            "Nat",
            &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
        ));

        // match n { Zero => 0 } — missing Succ arm
        let scrut = ast.push(AstNode::new("var").with_payload(json!("n")));
        let zero_pat = ast.push(AstNode::new("var").with_payload(json!("Zero")));
        let zero_body = ast.push(AstNode::new("lit").with_payload(json!(0)));
        let arm1 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [zero_pat, zero_body]})));
        let block = ast.push(AstNode::new("block").with_payload(json!({"children": [arm1]})));
        ast.push(AstNode::new("match").with_payload(json!({"children": [scrut, block]})));

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "match",
            "incomplete match should not be desugared"
        );
    }

    #[test]
    fn match_no_data_decl_not_desugared() {
        // match n { Foo => 0, Bar => 1 } — constructors "Foo" and "Bar" are
        // not declared in any `data` node. Should NOT desugar (no recursor).
        let mut lang = Language::new();
        lang.merge_ast_fragment(AstShapeDef::new("ast.var", "var")).unwrap();
        lang.merge_ast_fragment(AstShapeDef::new("ast.lit", "lit")).unwrap();
        lang.ast
            .merge(AstShapeDef::new("ast.match", "match"))
            .unwrap();

        let mut ast = Ast::new();
        let scrut = ast.push(AstNode::new("var").with_payload(json!("n")));
        let foo_pat = ast.push(AstNode::new("var").with_payload(json!("Foo")));
        let foo_body = ast.push(AstNode::new("lit").with_payload(json!(0)));
        let arm1 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [foo_pat, foo_body]})));
        let bar_pat = ast.push(AstNode::new("var").with_payload(json!("Bar")));
        let bar_body = ast.push(AstNode::new("lit").with_payload(json!(1)));
        let arm2 =
            ast.push(AstNode::new("arm").with_payload(json!({"children": [bar_pat, bar_body]})));
        let block = ast.push(AstNode::new("block").with_payload(json!({"children": [arm1, arm2]})));
        ast.push(AstNode::new("match").with_payload(json!({"children": [scrut, block]})));

        let out = apply_desugar(&lang, &ast);
        let root = out.nodes().last().unwrap();
        assert_eq!(
            root.kind, "match",
            "match with unknown constructors should not be desugared"
        );
    }

    #[test]
    fn desugar_registry_contains_expected_entries() {
        // The registry must be table-driven and contain the v0.1 sugar
        // forms. `if` is intentionally NOT registered (see desugar_registry
        // docs). This test guards against accidental removal of entries.
        let reg = desugar_registry();
        assert!(reg.contains_key("nullish"));
        assert!(reg.contains_key("opt-chain"));
        assert!(reg.contains_key("while"));
        assert!(reg.contains_key("do-while"));
        assert!(reg.contains_key("for"));
        assert!(reg.contains_key("data"));
        assert!(reg.contains_key("match"));
        // `if` is deliberately excluded — the typechecker has an `if`-specific
        // type rule that desugaring to `match` would bypass.
        assert!(
            !reg.contains_key("if"),
            "`if` must not be in the desugar registry (typechecker has an if-specific type rule)"
        );
    }
}
