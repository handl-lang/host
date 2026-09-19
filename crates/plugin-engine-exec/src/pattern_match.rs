//! Shared pattern matching logic for `match`/`handle` prims (ADR-0061).
//!
//! Pattern matching interprets a pattern AST node structurally against a
//! scrutinee value. The six pattern forms are:
//!
//! 1. **Wildcard** — `var` node named `_` or `_*` → always matches.
//! 2. **Binder** — `var` node with another name → binds name to scrutinee,
//!    always matches.
//! 3. **Literal** — any other node → evaluate and compare by `==`.
//! 4. **Array destructuring** — `array` node with sub-pattern children +
//!    optional `rest` node → positional match, `...rest` captures tail.
//! 5. **Object destructuring** — `object` node with `entry` children +
//!    optional `rest` node → key match, `...rest` captures unmatched keys.
//! 6. **Tag pattern** — `call` node with a `var` callee (the tag name) and
//!    `var` arg nodes (the binders): `ok(v)`, `err(e)`. Dispatches on
//!    `__tag__` identity and binds args to payload fields (ADR-0077).
//!
//! This module is engine-agnostic. Each engine implements
//! [`PatternMatchCtx`] to provide the two engine-specific operations:
//! binding a name in the scope stack and evaluating a pattern node as a
//! runtime value (for literal patterns). All pattern-node-kind dispatch
//! lives here, eliminating the 3x duplication across tree-walk, bytecode,
//! and cranelift engines.

use handl_language::{Ast, AstNode, AstRef, LangError, Language, Value};

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

    /// Resolve an ambient global binding by name (the engine's `globals`
    /// map — e.g. `true`/`false` seeded from `Language::globals`).
    ///
    /// A `var` pattern whose name resolves to a global is a *value
    /// pattern*: it matches by `==` instead of binding (`match(v) { true:
    /// "yes" }`). Only globals participate — a `var` bound in an ordinary
    /// scope still binds (binders shadow). The default returns `None`:
    /// engines without ambient globals keep bind-everything semantics.
    fn global_value(&self, _name: &str) -> Option<Value> {
        None
    }

    /// The language whose `AstShapeDef`s resolve pattern roles and child
    /// roles (ADR-0081).
    ///
    /// `None` — the default, e.g. a context with no language wired (the
    /// standalone cranelift driver) — keeps the legacy kind-name
    /// fallbacks.
    fn lang(&self) -> Option<&Language> {
        None
    }
}

/// Whether `node` plays `role` in pattern position (ADR-0081).
///
/// The shape's declared `pattern_role` is authoritative when one is
/// declared; a shape with no `pattern_role` — or a context with no
/// language wired — falls back to the legacy `kind == legacy_kind`
/// check, the same convention [`Language::literal_child`] uses for
/// role-less shapes.
fn pattern_role_is(lang: Option<&Language>, node: &AstNode, role: &str, legacy_kind: &str) -> bool {
    lang.and_then(|l| l.pattern_role_is(&node.kind, role))
        .unwrap_or_else(|| node.kind == legacy_kind)
}

/// Whether `node` is a leaf binder pattern — `pattern_role: leaf`
/// (canonical `var`), falling back to `kind == "var"` for role-less
/// shapes.
fn is_binder_node(lang: Option<&Language>, node: &AstNode) -> bool {
    pattern_role_is(lang, node, "leaf", "var")
}

/// Whether `node` is a tag pattern — its shape declares a `callee`
/// child role (canonical `call`; ADR-0081), falling back to
/// `kind == "call"` for role-less shapes.
fn is_tag_pattern_node(lang: Option<&Language>, node: &AstNode) -> bool {
    lang.map(|l| matches!(l.child_role_index(&node.kind, "callee"), Ok(Some(_))))
        .unwrap_or(false)
        || node.kind == "call"
}

/// A node's child index for `role` — the declared role index when the
/// language is wired and declares it, `legacy` otherwise.
fn child_idx(lang: Option<&Language>, node: &AstNode, role: &str, legacy: usize) -> usize {
    lang.and_then(|l| l.child_role_index(&node.kind, role).ok().flatten())
        .unwrap_or(legacy)
}

/// Whether a `var` name in pattern position is a wildcard (binds nothing).
///
/// The surface convention is `_`/`_*`. `__fresh_`-prefixed names are
/// exempt: they are minted by the transform engine (`RewriteStep::Fresh`)
/// as desugar-internal binders — e.g. the `for` desugar's `...rec_tail`
/// rest binder — and must bind even though they start with `_`.
fn is_wildcard_name(name: &str) -> bool {
    name.starts_with('_') && !name.starts_with("__fresh_")
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
    let lang = ctx.lang();
    // Leaf binder pattern — `pattern_role: leaf` (canonical `var`; the
    // `var` kind name is the role-less fallback).
    if is_binder_node(lang, pattern_node) {
        if let Some(name) = pattern_node.payload.as_str() {
            if is_wildcard_name(name) {
                // Wildcard — always matches, binds nothing.
                return Ok(true);
            }
            // Value pattern — an ambient global (e.g. `true`/`false`
            // seeded from `Language::globals`) matches by equality rather
            // than binding.
            if let Some(global) = ctx.global_value(name) {
                return Ok(global == *scrut);
            }
            // Binder — always matches, binds name → scrutinee.
            ctx.define_binding(name, scrut.clone());
            return Ok(true);
        }
    }
    // Array destructuring pattern (`pattern_role: sequence`): [a, b, ...rest]
    if pattern_role_is(lang, pattern_node, "sequence", "array") {
        return try_array_pattern(ast, pattern_ref, scrut, ctx);
    }
    // Object destructuring pattern (`pattern_role: entries`):
    // { x: a, y: b, ...rest }
    if pattern_role_is(lang, pattern_node, "entries", "object") {
        return try_object_pattern(ast, pattern_ref, scrut, ctx);
    }
    // Tag pattern: `tag_name(binder1, binder2, ...)` — a callee-roled
    // node (canonical `call`) with a `var` callee. Dispatch on `__tag__`
    // identity, bind args to payload fields (ADR-0077).
    if is_tag_pattern_node(lang, pattern_node) {
        return try_tag_pattern(ast, pattern_ref, scrut, ctx);
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
fn rest_binder_name(
    ast: &Ast,
    lang: Option<&Language>,
    rest_ref: AstRef,
) -> Result<Option<String>, LangError> {
    let rest_node = ast.at(rest_ref)?;
    let children = rest_node
        .payload
        .get("children")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            LangError::engine_node("rest pattern: missing children".to_string(), &rest_node)
        })?;
    // The binder sits at the `subject` role (index 0 on canonical `rest`).
    let subject_idx = child_idx(lang, rest_node, "subject", 0);
    let var_idx = children
        .get(subject_idx)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            LangError::engine_node("rest pattern: missing binder".to_string(), &rest_node)
        })?;
    let var_node = ast.at(AstRef::new(var_idx as usize))?;
    if is_binder_node(lang, var_node) {
        if let Some(name) = var_node.payload.as_str() {
            if is_wildcard_name(name) {
                return Ok(None); // Wildcard rest — binds nothing.
            }
            return Ok(Some(name.to_string()));
        }
    }
    Err(LangError::engine_node(
        "rest pattern: binder is not a var".to_string(),
        &rest_node,
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
        .ok_or_else(|| {
            LangError::engine_node("array pattern: missing children".to_string(), &pattern_node)
        })?;
    let scrut_arr = match scrut.as_array() {
        Some(a) => a,
        None => return Ok(false),
    };

    // Separate positional patterns from the optional rest pattern.
    let mut rest_idx: Option<usize> = None;
    let mut positional: Vec<usize> = Vec::new();
    for child in children {
        let idx = child.as_u64().ok_or_else(|| {
            LangError::engine_node("array pattern: bad child index".to_string(), &pattern_node)
        })? as usize;
        let child_node = ast.at(AstRef::new(idx))?;
        if pattern_role_is(ctx.lang(), child_node, "rest", "rest") {
            if rest_idx.is_some() {
                return Err(LangError::engine_node(
                    "array pattern: multiple rest patterns".to_string(),
                    &pattern_node,
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
        if let Some(name) = rest_binder_name(ast, ctx.lang(), AstRef::new(ridx))? {
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
        .ok_or_else(|| {
            LangError::engine_node(
                "object pattern: missing children".to_string(),
                &pattern_node,
            )
        })?;
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
            .ok_or_else(|| LangError::engine("object pattern: bad child index".to_string()))?
            as usize;
        let child_node = ast.at(AstRef::new(idx))?;
        if pattern_role_is(ctx.lang(), child_node, "rest", "rest") {
            if rest_idx.is_some() {
                return Err(LangError::engine(
                    "object pattern: multiple rest patterns".to_string(),
                ));
            }
            rest_idx = Some(idx);
        } else if pattern_role_is(ctx.lang(), child_node, "entry", "entry") {
            let entry_children = child_node
                .payload
                .get("children")
                .and_then(|v| v.as_array())
                .ok_or_else(|| LangError::engine("object pattern: malformed entry".to_string()))?;
            // The key and value-pattern sit at the entry shape's
            // `key`/`value` roles (indexes 0/1 on canonical `entry`).
            let key_idx = entry_children
                .get(child_idx(ctx.lang(), child_node, "key", 0))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::engine("object pattern: bad entry key index".to_string())
                })? as usize;
            let val_idx = entry_children
                .get(child_idx(ctx.lang(), child_node, "value", 1))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::engine("object pattern: bad entry val index".to_string())
                })? as usize;
            let key_node = ast.at(AstRef::new(key_idx))?;
            // A literal `key` child (declared via `literal_children`,
            // canonical `var`/`string`) carries the field name in its
            // payload; any other key expression is evaluated to get the
            // field name. `kind == "var"` is the role-less fallback.
            let is_literal_key = ctx
                .lang()
                .and_then(|l| l.literal_child(&child_node.kind, "key", &key_node.kind))
                .unwrap_or_else(|| key_node.kind == "var");
            let key = if is_literal_key {
                key_node.payload.as_str().unwrap_or("").to_string()
            } else {
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
        if let Some(name) = rest_binder_name(ast, ctx.lang(), AstRef::new(ridx))? {
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

/// Tag pattern: `tag_name(binder1, binder2, ...)` (ADR-0077).
///
/// The pattern is a `call` node with a `var` callee (the tag name) and
/// `var` arg nodes (the binders). Matches if the scrutinee is a tagged
/// value (`__tag__` key) whose tag name equals the callee name. Binds
/// each binder to the corresponding payload field:
/// - Single binder + single-field payload (`{value: v}`): bind to `value`.
/// - Multiple binders + multi-field payload: bind each to the field at
///   the corresponding position (alphabetical key order, excluding
///   `__tag__`).
/// - Wildcard binder (`_`): matches without binding.
fn try_tag_pattern(
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
        .ok_or_else(|| {
            LangError::engine_node("tag pattern: missing children".to_string(), &pattern_node)
        })?;
    // The `callee` role (index 0 on canonical `call`) carries the tag
    // name; the `args` role (index 1) and onward are the binders.
    let callee_idx = children
        .get(child_idx(ctx.lang(), pattern_node, "callee", 0))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            LangError::engine_node("tag pattern: missing callee".to_string(), &pattern_node)
        })? as usize;
    let callee_node = ast.at(AstRef::new(callee_idx))?;
    let tag_name = callee_node.payload.as_str().ok_or_else(|| {
        LangError::engine_node(
            "tag pattern: callee is not a var".to_string(),
            &pattern_node,
        )
    })?;
    // Check scrutinee is a tagged value with matching tag name.
    let scrut_obj = match scrut.as_object() {
        Some(o) => o,
        None => return Ok(false),
    };
    let scrut_tag = scrut_obj.get("__tag__").and_then(|v| v.as_str());
    match scrut_tag {
        Some(t) if t == tag_name => {}
        _ => return Ok(false),
    }
    // Collect payload fields (excluding __tag__), sorted alphabetically
    // for deterministic positional binding.
    let mut payload_fields: Vec<(&String, &Value)> = scrut_obj
        .iter()
        .filter(|(k, _)| k.as_str() != "__tag__")
        .collect();
    payload_fields.sort_by(|a, b| a.0.cmp(b.0));
    // Collect binder nodes (the `args` role onward; index 1 on canonical
    // `call`).
    let binder_indices: Vec<usize> = children
        .iter()
        .skip(child_idx(ctx.lang(), pattern_node, "args", 1))
        .map(|c| -> Result<usize, LangError> {
            let idx = c.as_u64().ok_or_else(|| {
                LangError::engine_node("tag pattern: bad binder index".to_string(), &pattern_node)
            })?;
            Ok(idx as usize)
        })
        .collect::<Result<_, _>>()?;
    // If there's a single binder and a single field, bind directly
    // (the common case for `ok(v)`, `err(e)`).
    if binder_indices.len() == 1 && payload_fields.len() == 1 {
        let binder_idx = binder_indices[0];
        let binder_node = ast.at(AstRef::new(binder_idx))?;
        if is_binder_node(ctx.lang(), binder_node) {
            if let Some(name) = binder_node.payload.as_str() {
                if is_wildcard_name(name) {
                    return Ok(true); // Wildcard
                }
                ctx.define_binding(name, payload_fields[0].1.clone());
                return Ok(true);
            }
        }
    }
    // General case: positional binding of payload fields to binders.
    for (i, &binder_idx) in binder_indices.iter().enumerate() {
        let binder_node = ast.at(AstRef::new(binder_idx))?;
        if is_binder_node(ctx.lang(), binder_node) {
            if let Some(name) = binder_node.payload.as_str() {
                if is_wildcard_name(name) {
                    continue; // Wildcard
                }
                if let Some(&(_, val)) = payload_fields.get(i) {
                    ctx.define_binding(name, val.clone());
                }
            }
        }
    }
    Ok(true)
}
