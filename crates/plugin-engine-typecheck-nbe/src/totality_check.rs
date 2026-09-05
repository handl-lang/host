//! Totality checking via structural recursion analysis (Phase 7.1).
//!
//! When a function is annotated `Total(A -> B)`, the typechecker checks
//! whether the function is total (terminates on all inputs) by verifying
//! structural recursion:
//!
//! - **Generated recursors**: if the function node carries `termination`
//!   metadata (via [`read_termination_metadata`]), it is recognized as
//!   structurally recursive — the `data` desugaring guarantees termination.
//! - **User-written recursive functions**: a syntactic check finds all
//!   recursive calls (calls to the function being checked within its body)
//!   and verifies each call's argument is a **strict sub-structure** of a
//!   parameter (e.g. `n.pred` is a sub-structure of `n`). "Strict
//!   sub-structure" means: the argument is a chain of `field-access` nodes
//!   ending in a parameter variable.
//! - **Non-recursive functions**: trivially total (no recursive calls →
//!   no infinite recursion).
//!
//! This is a conservative check: it only recognizes field-access decreasing
//! (matching the inductive-type pattern). Arithmetic decreasing (e.g.
//! `n - 1`) is not recognized — such functions are Disproved by this simple
//! check, which is acceptable per the Phase 7 design.

use handl_language::{Ast, AstNode, AstRef, Language, Proof};

use super::read_termination_metadata;

/// Check whether a function is total (terminates on all inputs).
///
/// `expr_ref` points to the expression being annotated `Total(...)`. This
/// may be:
/// - A `function` node (anonymous function literal).
/// - An `assign` node (named function definition: `f = (x) -> ...`).
/// - A `var` node (named function reference — we look up the binding in
///   the AST to find the function definition).
///
/// Returns [`Proof::ok`] if the function is total, [`Proof::no`] if it is
/// not (a recursive call doesn't decrease), or [`Proof::blocked`] if the
/// check cannot be performed (e.g. the expression is not a function).
pub fn check_totality(ast: &Ast, expr_ref: AstRef, _lang: &Language) -> Proof {
    let node = match ast.get(expr_ref) {
        Some(n) => n,
        None => return Proof::blocked("totality: expression node out of bounds"),
    };

    match node.kind.as_str() {
        "function" => check_function_totality(ast, node, None),
        "assign" => check_assign_totality(ast, node),
        "var" => {
            // Named function reference: look up the assignment in the AST
            // by scanning for an `assign` whose target var matches this name.
            let name = match node.payload.as_str() {
                Some(n) => n,
                None => return Proof::blocked("totality: var node has no string payload"),
            };
            find_assign_for_name(ast, name)
                .and_then(|assign_ref| {
                    let assign_node = ast.get(assign_ref)?;
                    let fn_name = extract_assign_name(ast, assign_node);
                    let fn_node = extract_assign_value(ast, assign_node)?;
                    Some(check_function_totality(ast, fn_node, fn_name.as_deref()))
                })
                .unwrap_or_else(|| {
                    Proof::blocked("totality: could not find function definition for var")
                })
        }
        _ => {
            // Not a function — can't check totality.
            Proof::blocked(format!(
                "totality: expression kind '{}' is not a function",
                node.kind
            ))
        }
    }
}

/// Check totality of a `function` node.
///
/// `fn_name` is the name of the function (if named via `assign`), used to
/// find recursive calls. If `None`, the function is anonymous and has no
/// recursive calls (trivially total).
fn check_function_totality(ast: &Ast, fn_node: &AstNode, fn_name: Option<&str>) -> Proof {
    // 1. Check for termination metadata (generated recursors).
    if let Some(info) = read_termination_metadata(fn_node) {
        return Proof::ok(format!(
            "structurally recursive via recursor metadata (decreasing_arg={}, field='{}')",
            info.decreasing_arg, info.decreasing_field
        ));
    }

    // 2. Extract param names and body index.
    let (param_names, body_idx) = match function_param_body(ast, fn_node) {
        Some(v) => v,
        None => return Proof::blocked("totality: function has invalid params/body"),
    };

    // 3. If the function is anonymous (no name), there are no recursive
    //    calls — it's trivially total.
    let fn_name = match fn_name {
        Some(n) => n,
        None => {
            return Proof::ok("non-recursive, trivially total (anonymous function)");
        }
    };

    // 4. Find all recursive calls in the body and check decreasing.
    let body_ref = AstRef::new(body_idx);
    let mut calls = Vec::new();
    find_recursive_calls(ast, body_ref, fn_name, &mut calls);

    if calls.is_empty() {
        return Proof::ok("non-recursive, trivially total");
    }

    // 5. For each recursive call, check if at least one argument is a
    //    strict sub-structure of a parameter.
    for call_ref in &calls {
        if !call_decreases_on_params(ast, *call_ref, &param_names) {
            return Proof::no(format!(
                "recursive call to '{}' does not decrease on a parameter (not a strict sub-structure)",
                fn_name
            ));
        }
    }

    Proof::ok(format!(
        "structurally recursive: all {} recursive call(s) decrease on a parameter",
        calls.len()
    ))
}

/// Check totality of an `assign` node (named function definition).
///
/// The assign node has a target (the function name) and a value (the
/// function node). We extract the name and function, then delegate to
/// [`check_function_totality`].
fn check_assign_totality(ast: &Ast, assign_node: &AstNode) -> Proof {
    let fn_name = extract_assign_name(ast, assign_node);
    let fn_node = match extract_assign_value(ast, assign_node) {
        Some(n) => n,
        None => return Proof::blocked("totality: assign has no value child"),
    };

    if fn_node.kind != "function" {
        return Proof::blocked(format!(
            "totality: assign value is not a function, got '{}'",
            fn_node.kind
        ));
    }

    check_function_totality(ast, fn_node, fn_name.as_deref())
}

/// Extract the function name from an `assign` node's target child.
fn extract_assign_name(ast: &Ast, assign_node: &AstNode) -> Option<String> {
    // Try role-based resolution first (target role), then positional.
    let children = assign_node.payload.get("children")?.as_array()?;
    let target_idx = children.first()?.as_u64()? as usize;
    let target_node = ast.get(AstRef::new(target_idx))?;
    if target_node.kind == "var" {
        target_node.payload.as_str().map(|s| s.to_string())
    } else {
        None
    }
}

/// Extract the value (function) node from an `assign` node.
fn extract_assign_value<'a>(ast: &'a Ast, assign_node: &'a AstNode) -> Option<&'a AstNode> {
    let children = assign_node.payload.get("children")?.as_array()?;
    let value_idx = children.get(1)?.as_u64()? as usize;
    ast.get(AstRef::new(value_idx))
}

/// Find an `assign` node in the AST whose target var matches `name`.
fn find_assign_for_name(ast: &Ast, name: &str) -> Option<AstRef> {
    for (idx, node) in ast.nodes().iter().enumerate() {
        if node.kind == "assign" {
            if let Some(n) = extract_assign_name(ast, node) {
                if n == name {
                    return Some(AstRef::new(idx));
                }
            }
        }
    }
    None
}

/// Extract `(param_names, body_idx)` from a `function` node.
///
/// Handles both the rich format (`{params: [...], body: idx}`) and the
/// legacy format (`{children: [param_idx, body_idx]}`).
fn function_param_body(ast: &Ast, node: &AstNode) -> Option<(Vec<String>, usize)> {
    // Rich format: {params: ["name", ...], body: idx}
    if let (Some(params), Some(body)) = (
        node.payload.get("params").and_then(|v| v.as_array()),
        node.payload.get("body").and_then(|v| v.as_u64()),
    ) {
        let names: Vec<String> = params
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        return Some((names, body as usize));
    }
    // Legacy format: {children: [param_idx, body_idx]}
    let children = node.payload.get("children").and_then(|c| c.as_array())?;
    if children.len() < 2 {
        return None;
    }
    let param_idx = children[0].as_u64()? as usize;
    let body_idx = children[1].as_u64()? as usize;
    let param_node = ast.get(AstRef::new(param_idx))?;
    let names = if param_node.kind == "var" {
        vec![param_node.payload.as_str()?.to_string()]
    } else {
        let param_children = param_node.payload.get("children").and_then(|c| c.as_array())?;
        param_children
            .iter()
            .filter_map(|pc| {
                let pidx = pc.as_u64()? as usize;
                ast.get(AstRef::new(pidx))
                    .and_then(|pn| pn.payload.as_str())
                    .map(String::from)
            })
            .collect()
    };
    Some((names, body_idx))
}

/// Recursively find all `call` nodes in a subtree that call `fn_name`.
///
/// Populates `results` with the [`AstRef`]s of the matching `call` nodes.
fn find_recursive_calls(ast: &Ast, node_ref: AstRef, fn_name: &str, results: &mut Vec<AstRef>) {
    let node = match ast.get(node_ref) {
        Some(n) => n,
        None => return,
    };

    if node.kind == "call" {
        // Check if the callee is a var with the function's name.
        if call_callee_is_var_named(ast, node, fn_name) {
            results.push(node_ref);
        }
    }

    // Recurse into all children.
    if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
        for child_val in children {
            if let Some(child_idx) = child_val.as_u64() {
                find_recursive_calls(ast, AstRef::new(child_idx as usize), fn_name, results);
            }
        }
    }
}

/// Check if a `call` node's callee (child 0) is a `var` with the given name.
fn call_callee_is_var_named(ast: &Ast, call_node: &AstNode, name: &str) -> bool {
    let children = match call_node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => return false,
    };
    let callee_idx = match children.first().and_then(|v| v.as_u64()) {
        Some(i) => i as usize,
        None => return false,
    };
    let callee_node = match ast.get(AstRef::new(callee_idx)) {
        Some(n) => n,
        None => return false,
    };
    callee_node.kind == "var" && callee_node.payload.as_str() == Some(name)
}

/// Check if a recursive `call` node decreases on at least one parameter.
///
/// A call decreases if at least one argument is a **strict sub-structure**
/// of a parameter: a chain of `field-access` nodes ending in a `var` that
/// names a parameter.
fn call_decreases_on_params(ast: &Ast, call_ref: AstRef, param_names: &[String]) -> bool {
    let call_node = match ast.get(call_ref) {
        Some(n) => n,
        None => return false,
    };
    let children = match call_node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => return false,
    };
    // Skip child 0 (the callee); check children 1..N (the arguments).
    for arg_val in children.iter().skip(1) {
        if let Some(arg_idx) = arg_val.as_u64() {
            if is_strict_sub_structure(ast, AstRef::new(arg_idx as usize), param_names) {
                return true;
            }
        }
    }
    false
}

/// Check if an expression is a strict sub-structure of a parameter.
///
/// A strict sub-structure is a chain of `field-access` nodes ending in a
/// `var` that names a parameter. For example:
/// - `n.pred` → `field-access(var("n"), "pred")` → strict sub-structure of `n` ✓
/// - `n.pred.pred` → nested field-access → strict sub-structure ✓
/// - `n` (bare parameter) → NOT a strict sub-structure ✗
/// - `n - 1` → `binary.sub(...)` → NOT a field-access ✗
fn is_strict_sub_structure(ast: &Ast, node_ref: AstRef, param_names: &[String]) -> bool {
    let node = match ast.get(node_ref) {
        Some(n) => n,
        None => return false,
    };

    if node.kind != "field-access" {
        // Not a field-access → not a strict sub-structure.
        return false;
    }

    // Get the target of the field-access.
    let target_ref = match field_access_target(ast, node) {
        Some(r) => r,
        None => return false,
    };

    let target_node = match ast.get(target_ref) {
        Some(n) => n,
        None => return false,
    };

    match target_node.kind.as_str() {
        "var" => {
            // Check if the var names a parameter.
            let name = target_node.payload.as_str();
            match name {
                Some(n) => param_names.iter().any(|p| p == n),
                None => false,
            }
        }
        "field-access" => {
            // Recursively check if the target is a strict sub-structure.
            is_strict_sub_structure(ast, target_ref, param_names)
        }
        _ => false,
    }
}

/// Extract the target child ref from a `field-access` node.
///
/// Handles both the `{"field": "name", "children": [target_idx]}` format
/// and the `{"children": [target_idx, field_idx]}` format.
fn field_access_target(ast: &Ast, node: &AstNode) -> Option<AstRef> {
    let children = node.payload.get("children")?.as_array()?;
    let target_idx = children.first()?.as_u64()? as usize;
    // Verify the child exists.
    ast.get(AstRef::new(target_idx))?;
    Some(AstRef::new(target_idx))
}
