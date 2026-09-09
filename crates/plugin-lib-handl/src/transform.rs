//! Handl-authored transform rules (ADR-0094 — the `Handl(rule_id)` seam).
//!
//! A `TransformDef` can name a `.hnl` function as its rewrite using the
//! [`NativeRewrite`] dispatch spelling with a `handl:` name prefix:
//!
//! ```yaml
//! - id: transform.mypass.foo
//!   pass: mypass
//!   node_kind: foo
//!   rule: !Native "handl:mymod.myfn"
//! ```
//!
//! `handl:mymod.myfn` resolves to function `myfn` on the [`HandlModule`]
//! registered under module name `mymod` with a [`HandlNativeRewrite`]
//! registry. The registry itself is a [`NativeRewrite`] — wire it into a
//! `RewriteEngine` via `with_native_registry`, optionally layering a
//! fallback registry (e.g. the `desugar.*` natives) underneath so
//! non-`handl:` names keep resolving.
//!
//! ## Value contract
//!
//! The `.hnl` function is called with **one argument**: a JSON-shaped
//! record serialization of the matched node —
//!
//! ```text
//! { kind: "foo",
//!   span: { start: 0, end: 3 } | null,
//!   payload: { … },               -- the node's payload minus `children`
//!   children: [ <node record>, … ] -- recursively serialized children
//! }
//! ```
//!
//! It returns either:
//!
//! - `null` — decline the rewrite; the engine pushes the node through
//!   unchanged (children remapped), exactly like a native returning the
//!   pass-through path.
//! - a **node spec** record `{ kind, payload?, children? }` — a fresh node
//!   tree the engine realizes into the output arena. `payload` is used
//!   verbatim (excluding `children`, which is set from the realized
//!   `children` list). Every element of `children` is itself a node spec,
//!   **or** the marker `{ "__keep__": n }` which reuses the remapped
//!   output of the *matched* node's child `n` (i.e. `n` indexes the
//!   original node's `children` array, no matter how deeply nested the
//!   marker appears). A bare `{ "__keep__": n }` return replaces the
//!   matched node with that child.
//!
//! Realized nodes inherit the matched node's `span` — the engine's
//! documented span policy for rewrites.
//!
//! ## Notes / rough edges
//!
//! - The `.hnl` function cannot observe or produce raw arena indices —
//!   `__keep__` is the only handle onto existing subtrees.
//! - Invocation goes through [`HandlFunction`]'s per-thread cached bridge
//!   `Runtime` (the `pred.hnl`/`plugin-engine-test` pattern): the node
//!   record crosses as an `__arg_0` global and the result returns through
//!   the serde_json boundary, so the function must not return arrow values
//!   or other reserved-key (`__arrow__`/`__tag__`/…) shapes.
//! - The bridge runtime runs its own `desugar` pass when evaluating
//!   `(source).fn(arg)`; since it builds its own canonical `Language`,
//!   user-authored `handl:` rules never fire inside the bridge — no
//!   recursion through the transform engine.

use std::collections::HashMap;
use std::sync::Arc;

use handl_language::{Ast, AstNode, AstRef, LangError};
use handl_platform::{EvalContext, Module};
use handl_plugin_engine_transform::native::{NativeRewrite, RewriteCtx};
use serde_json::Value;

use crate::HandlModule;

/// The `TransformRule::Native` name prefix that routes a rule to a
/// `.hnl` function: `"handl:<module>.<fn>"`.
pub const HANDL_PREFIX: &str = "handl:";

/// The marker key a `.hnl` transform returns to reuse a remapped child of
/// the matched node: `{ "__keep__": n }`.
pub const KEEP_KEY: &str = "__keep__";

/// The marker key a `.hnl` transform returns to request a fresh `var` leaf:
/// `{ "__fresh__": "hint" }`.
pub const FRESH_KEY: &str = "__fresh__";

/// A [`NativeRewrite`] that resolves `handl:<module>.<fn>` rule names
/// against registered [`HandlModule`]s and invokes the `.hnl` function.
///
/// Non-`handl:` names delegate to the optional fallback registry, so a
/// single `with_native_registry` call can serve both handl-authored rules
/// and Rust natives (e.g. `desugar.*`):
///
/// ```ignore
/// let natives = HandlNativeRewrite::new()
///     .with_module(HandlModule::new("t", MY_HNL, &["rewrite"]))
///     .with_fallback(Arc::new(desugar_native_registry()));
/// let engine = RewriteEngine::new().with_native_registry(Arc::new(natives));
/// ```
pub struct HandlNativeRewrite {
    /// Module name → module providing the rewrite functions.
    modules: HashMap<String, Arc<HandlModule>>,
    /// Fallback registry for names without the `handl:` prefix.
    fallback: Option<Arc<dyn NativeRewrite>>,
}

impl Default for HandlNativeRewrite {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlNativeRewrite {
    /// Create an empty registry (no modules, no fallback).
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            fallback: None,
        }
    }

    /// Register a [`HandlModule`] under its module name. Functions are
    /// resolved lazily at rewrite time (`module.function(fn_name)`).
    pub fn with_module(mut self, module: HandlModule) -> Self {
        self.modules
            .insert(module.name().to_string(), Arc::new(module));
        self
    }

    /// Register a pre-`Arc`ed [`HandlModule`].
    pub fn with_module_arc(mut self, module: Arc<HandlModule>) -> Self {
        self.modules.insert(module.name().to_string(), module);
        self
    }

    /// Wire a fallback [`NativeRewrite`] consulted for names that do not
    /// carry the `handl:` prefix (e.g. the `desugar.*` registry).
    pub fn with_fallback(mut self, fallback: Arc<dyn NativeRewrite>) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

impl NativeRewrite for HandlNativeRewrite {
    fn rewrite(&self, name: &str, ctx: &mut RewriteCtx<'_>) -> Result<AstRef, LangError> {
        let rest = match name.strip_prefix(HANDL_PREFIX) {
            Some(rest) => rest,
            None => {
                return match &self.fallback {
                    Some(f) => f.rewrite(name, ctx),
                    None => Err(LangError::engine(format!(
                        "unknown native rewrite: {name} (no handl module or fallback registered)"
                    ))),
                };
            }
        };
        let (module_name, fn_name) = rest.rsplit_once('.').ok_or_else(|| {
            LangError::engine(format!(
                "handl transform name '{name}' must be 'handl:<module>.<fn>'"
            ))
        })?;
        let module = self.modules.get(module_name).ok_or_else(|| {
            LangError::engine(format!(
                "handl transform '{name}': no HandlModule registered under '{module_name}'"
            ))
        })?;
        let function = module.function(fn_name).ok_or_else(|| {
            LangError::engine(format!(
                "handl transform '{name}': module '{module_name}' has no function '{fn_name}'"
            ))
        })?;

        // Serialize the matched node (recursively) into the value contract.
        let node_value = serialize_node_at(ctx.ast(), ctx.node());

        let result = function
            .call(&[node_value], &mut EvalContext::new())
            .map_err(|e| LangError::engine(format!("handl transform '{name}' failed: {e}")))?;

        // `null` declines: pass the node through with children remapped.
        if result.is_null() {
            return Ok(ctx.pass_through());
        }
        let mut fresh_counter = 0usize;
        let mut fresh_names = HashMap::new();
        let idx = realize(&result, ctx, &mut fresh_counter, &mut fresh_names)
            .map_err(|e| LangError::engine(format!("handl transform '{name}': {e}")))?;
        Ok(AstRef::new(idx))
    }
}

/// Serialize `node` in `ast` into the `{kind, span, payload, children}`
/// value contract (children recursively).
fn serialize_node_at(ast: &Ast, node: &AstNode) -> Value {
    let mut payload = node.payload.clone();
    let mut children = Vec::new();
    if let Some(arr) = node.payload.get("children").and_then(|c| c.as_array()) {
        for child_ref in arr {
            if let Some(idx) = child_ref.as_u64() {
                if let Some(child) = ast.get(AstRef::new(idx as usize)) {
                    children.push(serialize_node_at(ast, child));
                }
            }
        }
    }
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("children");
    }
    serde_json::json!({
        "kind": node.kind,
        "span": node.span.map(|s| serde_json::json!({"start": s.start, "end": s.end})).unwrap_or(Value::Null),
        "payload": payload,
        "children": children,
    })
}

/// Realize a returned node spec into the output arena.
///
/// `{"__keep__": n}` reuses the matched node's remapped child `n`;
/// `{"__fresh__": "hint"}` mints a unique `var` leaf (reused for the same
/// hint within one rule application);
/// otherwise `{kind, payload?, children?}` builds a fresh node
/// (children realized recursively, `__keep__` / `__fresh__` still indexing
/// the matched node's children). Realized nodes inherit the matched node's span.
fn realize(
    spec: &Value,
    ctx: &mut RewriteCtx<'_>,
    fresh_counter: &mut usize,
    fresh_names: &mut HashMap<String, usize>,
) -> Result<usize, LangError> {
    if let Some(keep) = spec.get(KEEP_KEY) {
        let i = keep.as_u64().ok_or_else(|| {
            LangError::engine(format!(
                "'{KEEP_KEY}' marker must be a child index, got {keep}"
            ))
        })? as usize;
        return ctx.remap_child(i).ok_or_else(|| {
            LangError::engine(format!(
                "'{KEEP_KEY}' marker references child {i} which has no remapped output"
            ))
        });
    }
    if let Some(hint) = spec.get(FRESH_KEY) {
        let hint = hint.as_str().ok_or_else(|| {
            LangError::engine(format!(
                "'{FRESH_KEY}' marker must be a string hint, got {hint}"
            ))
        })?;
        if let Some(&idx) = fresh_names.get(hint) {
            return Ok(idx);
        }
        let name = format!("__fresh_{hint}_{fresh_counter}");
        *fresh_counter += 1;
        let mut node = AstNode::new("var").with_payload(Value::from(name));
        node.span = ctx.node().span;
        let idx = ctx.out().push(node).index();
        fresh_names.insert(hint.to_string(), idx);
        return Ok(idx);
    }
    let obj = spec.as_object().ok_or_else(|| {
        LangError::engine(format!(
            "handl transform must return null, a node spec record, a '{KEEP_KEY}' marker, or a '{FRESH_KEY}' marker; got {spec}"
        ))
    })?;
    let kind = obj
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            LangError::engine("node spec record requires a string 'kind' field".to_string())
        })?
        .to_string();
    let mut child_indices = Vec::new();
    if let Some(children) = obj.get("children").and_then(Value::as_array) {
        for child in children {
            child_indices.push(realize(child, ctx, fresh_counter, fresh_names)?);
        }
    }
    let mut payload = obj.get("payload").cloned().unwrap_or(Value::Null);
    if !child_indices.is_empty() {
        if !payload.is_object() {
            payload = Value::Object(serde_json::Map::new());
        }
        payload
            .as_object_mut()
            .unwrap()
            .insert("children".to_string(), Value::from(child_indices));
    }
    let mut node = AstNode::new(kind).with_payload(payload);
    node.span = ctx.node().span;
    Ok(ctx.out().push(node).index())
}
