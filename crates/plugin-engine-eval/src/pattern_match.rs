//! Shared pattern matching logic for `match`/`handle` prims (ADR-0061).
//!
//! Pattern matching interprets a pattern AST node structurally against a
//! scrutinee value. The five pattern forms are:
//!
//! 1. **Wildcard** — `var` node named `_` or `_*` → always matches.
//! 2. **Binder** — `var` node with another name → binds name to scrutinee,
//!    always matches.
//! 3. **Literal** — any other node → evaluate and compare by `==`.
//! 4. **Array destructuring** — `array` node with sub-pattern children +
//!    optional `rest` node → positional match, `...rest` captures tail.
//! 5. **Object destructuring** — `object` node with `entry` children +
//!    optional `rest` node → key match, `...rest` captures unmatched keys.
//!
//! This module is engine-agnostic. Each engine implements
//! [`PatternMatchCtx`] to provide the two engine-specific operations:
//! binding a name in the scope stack and evaluating a pattern node as a
//! runtime value (for literal patterns). All pattern-node-kind dispatch
//! lives here, eliminating the 3x duplication across tree-walk, bytecode,
//! and cranelift engines.

use handl_language::{Ast, AstRef, LangError, Value};

/// Engine-specific context for pattern matching (ADR-0061).
///
/// Abstracts the two operations pattern matching needs from the engine:
/// binding a name in the scope stack and evaluating a pattern AST node as
/// a runtime value. Each engine implements this trait and passes `&mut
/// self` to [`try_pattern`].
pub trait PatternMatchCtx {
    /// Bind `name` to `value` in the current (innermost) scope.
    ///
    /// Used by binder patterns (`var` node with a name) and rest patterns
    /// (`...rest` capturing remaining elements/keys). The caller is
    /// responsible for push/pop scope around the match attempt.
    fn define_binding(&mut self, name: &str, value: Value);

    /// Evaluate a pattern AST node as a runtime value.
    ///
    /// Used by literal patterns: the pattern node is evaluated as a value
    /// and compared by equality (`==`) to the scrutinee. Also used by
    /// object patterns to evaluate computed keys.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on evaluation failure.
    fn eval_pattern_node(&mut self, ast: &Ast, node_ref: AstRef) -> Result<Value, LangError>;
}

/// Interpret a pattern AST node structurally against a scrutinee value.
///
/// Returns `Ok(true)` if the pattern matches, `Ok(false)` if it does not.
/// Bindings are inserted via [`PatternMatchCtx::define_binding`]; the
/// caller is responsible for push/pop scope around the match attempt.
///
/// # Errors
///
/// Returns [`LangError`] if the pattern node is malformed or if literal
/// evaluation fails.
pub fn try_pattern(
    ast: &Ast,
    pattern_ref: AstRef,
    scrut: &Value,
    ctx: &mut impl PatternMatchCtx,
) -> Result<bool, LangError> {
    let pattern_node = ast.at(pattern_ref)?;
    // `var` node (identifier pattern).
    if pattern_node.kind == "var" {
        if let Some(name) = pattern_node.payload.as_str() {
            if name == "_" || name.starts_with('_') {
                // Wildcard — always matches, binds nothing.
                return Ok(true);
            }
            // Binder — always matches, binds name → scrutinee.
            ctx.define_binding(name, scrut.clone());
            return Ok(true);
        }
    }
    // Array destructuring pattern: [a, b, ...rest]
    if pattern_node.kind == "array" {
        return try_array_pattern(ast, pattern_ref, scrut, ctx);
    }
    // Object destructuring pattern: { x: a, y: b, ...rest }
    if pattern_node.kind == "object" {
        return try_object_pattern(ast, pattern_ref, scrut, ctx);
    }
    // Literal pattern — evaluate and compare by equality.
    let pat_val = ctx.eval_pattern_node(ast, pattern_ref)?;
    Ok(pat_val == *scrut)
}

/// Extract the binder name from a `rest` pattern node (`...name`).
///
/// Returns `Some(name)` if the rest node binds a name, or `None` if it's
/// a wildcard rest (`..._` or `..._*`). Returns `Err` if the rest node is
/// malformed.
fn rest_binder_name(ast: &Ast, rest_ref: AstRef) -> Result<Option<String>, LangError> {
    let rest_node = ast.at(rest_ref)?;
    let children = rest_node
        .payload
        .get("children")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LangError::Engine("rest pattern: missing children".to_string()))?;
    let var_idx = children
        .first()
        .and_then(|v| v.as_u64())
        .ok_or_else(|| LangError::Engine("rest pattern: missing binder".to_string()))?;
    let var_node = ast.at(AstRef::new(var_idx as usize))?;
    if var_node.kind == "var" {
        if let Some(name) = var_node.payload.as_str() {
            if name == "_" || name.starts_with('_') {
                return Ok(None); // Wildcard rest — binds nothing.
            }
            return Ok(Some(name.to_string()));
        }
    }
    Err(LangError::Engine(
        "rest pattern: binder is not a var".to_string(),
    ))
}

/// Array destructuring: `[a, b, ...rest]`.
///
/// Positional sub-patterns are matched left-to-right. A `...rest` element
/// captures the remaining scrutinee elements as an array bound to `rest`.
/// Without a rest, the array length must match exactly.
fn try_array_pattern(
    ast: &Ast,
    pattern_ref: AstRef,
    scrut: &Value,
    ctx: &mut impl PatternMatchCtx,
) -> Result<bool, LangError> {
    let pattern_node = ast.at(pattern_ref)?;
    let children = pattern_node
        .payload
        .get("children")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LangError::Engine("array pattern: missing children".to_string()))?;
    let scrut_arr = match scrut.as_array() {
        Some(a) => a,
        None => return Ok(false),
    };

    // Separate positional patterns from the optional rest pattern.
    let mut rest_idx: Option<usize> = None;
    let mut positional: Vec<usize> = Vec::new();
    for child in children {
        let idx = child
            .as_u64()
            .ok_or_else(|| LangError::Engine("array pattern: bad child index".to_string()))?
            as usize;
        let child_node = ast.at(AstRef::new(idx))?;
        if child_node.kind == "rest" {
            if rest_idx.is_some() {
                return Err(LangError::Engine(
                    "array pattern: multiple rest patterns".to_string(),
                ));
            }
            rest_idx = Some(idx);
        } else {
            positional.push(idx);
        }
    }

    let n = positional.len();
    if rest_idx.is_none() && scrut_arr.len() != n {
        return Ok(false); // Exact length required without rest.
    }
    if scrut_arr.len() < n {
        return Ok(false); // Not enough elements for positional patterns.
    }

    // Match positional sub-patterns.
    for (i, &pat_idx) in positional.iter().enumerate() {
        if !try_pattern(ast, AstRef::new(pat_idx), &scrut_arr[i], ctx)? {
            return Ok(false);
        }
    }

    // Bind remaining elements to the rest binder.
    if let Some(ridx) = rest_idx {
        if let Some(name) = rest_binder_name(ast, AstRef::new(ridx))? {
            let rest_vals: Vec<Value> = scrut_arr[n..].to_vec();
            ctx.define_binding(&name, Value::Array(rest_vals));
        }
    }

    Ok(true)
}

/// Object destructuring: `{ x: a, y: b, ...rest }`.
///
/// Each entry's key is a field name (from a `var` or evaluated to a
/// string); the entry's value is a sub-pattern matched against the
/// corresponding field. A `...rest` element captures the unmatched keys
/// as an object.
fn try_object_pattern(
    ast: &Ast,
    pattern_ref: AstRef,
    scrut: &Value,
    ctx: &mut impl PatternMatchCtx,
) -> Result<bool, LangError> {
    let pattern_node = ast.at(pattern_ref)?;
    let children = pattern_node
        .payload
        .get("children")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LangError::Engine("object pattern: missing children".to_string()))?;
    let scrut_obj = match scrut.as_object() {
        Some(o) => o,
        None => return Ok(false),
    };

    // Separate entries from the optional rest pattern.
    let mut rest_idx: Option<usize> = None;
    let mut entries: Vec<(String, usize)> = Vec::new(); // (key, value_pattern_idx)
    for child in children {
        let idx = child
            .as_u64()
            .ok_or_else(|| LangError::Engine("object pattern: bad child index".to_string()))?
            as usize;
        let child_node = ast.at(AstRef::new(idx))?;
        if child_node.kind == "rest" {
            if rest_idx.is_some() {
                return Err(LangError::Engine(
                    "object pattern: multiple rest patterns".to_string(),
                ));
            }
            rest_idx = Some(idx);
        } else if child_node.kind == "entry" {
            let entry_children = child_node
                .payload
                .get("children")
                .and_then(|v| v.as_array())
                .ok_or_else(|| LangError::Engine("object pattern: malformed entry".to_string()))?;
            let key_idx = entry_children[0]
                .as_u64()
                .ok_or_else(|| {
                    LangError::Engine("object pattern: bad entry key index".to_string())
                })? as usize;
            let val_idx = entry_children[1]
                .as_u64()
                .ok_or_else(|| {
                    LangError::Engine("object pattern: bad entry val index".to_string())
                })? as usize;
            let key_node = ast.at(AstRef::new(key_idx))?;
            // Key can be a `var` (identifier) or a `value` (string literal).
            let key = if key_node.kind == "var" {
                key_node.payload.as_str().unwrap_or("").to_string()
            } else {
                // Evaluate the key expression to get the field name.
                let key_val = ctx.eval_pattern_node(ast, AstRef::new(key_idx))?;
                key_val.as_str().unwrap_or("").to_string()
            };
            entries.push((key, val_idx));
        }
    }

    // Match each entry against the corresponding field.
    let mut matched_keys: Vec<String> = Vec::new();
    for (key, val_idx) in &entries {
        match scrut_obj.get(key) {
            Some(scrut_val) => {
                matched_keys.push(key.clone());
                if !try_pattern(ast, AstRef::new(*val_idx), scrut_val, ctx)? {
                    return Ok(false);
                }
            }
            None => return Ok(false), // Missing key — no match.
        }
    }

    // Bind remaining keys to the rest binder.
    if let Some(ridx) = rest_idx {
        if let Some(name) = rest_binder_name(ast, AstRef::new(ridx))? {
            let rest_obj: serde_json::Map<String, Value> = scrut_obj
                .iter()
                .filter(|(k, _)| !matched_keys.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            ctx.define_binding(&name, Value::Object(rest_obj));
        }
    }

    Ok(true)
}
