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
//!   children: [ <node record>, … ], -- recursively serialized children
//!   roles: { scrutinee: 0, … },    -- shape child-roles resolved for THIS
//!                                  -- node (arity-aware; ADR-0081)
//! }
//! ```
//!
//! The **top-level** record additionally carries
//!
//! ```text
//!   env: { self: <arena index of the matched node>,
//!          nodes: [ {index, kind, span, payload} … ] }
//! ```
//!
//! — a flat, index-addressed view of the whole *input* arena (payloads
//! verbatim, so `payload.children` are arena indices into `env.nodes`).
//! This is the generic declaration/environment service: a `.hnl` rewrite
//! that needs to see beyond its own subtree (e.g. `match` resolving
//! constructor names against the file's `data` declarations) navigates the
//! arena itself instead of asking the engine for a kind-specific lookup.
//!
//! It returns either:
//!
//! - `null` — decline the rewrite; the engine pushes the node through
//!   unchanged (children remapped), exactly like a native returning the
//!   pass-through path.
//! - a **node spec** record `{ kind, payload?, children? }` — a fresh node
//!   tree the engine realizes into the output arena. `payload` is used
//!   verbatim (excluding `children`, which is set from the realized
//!   `children` list) *except* that marker values inside the payload are
//!   resolved to arena indices (see below). Every element of `children`
//!   is itself a node spec or a marker:
//!
//!   - `{ "__keep__": n }` reuses the remapped output of the *matched*
//!     node's child `n` (`n` indexes the original node's `children`
//!     array, no matter how deeply nested the marker appears). A bare
//!     `{ "__keep__": n }` return replaces the matched node with that
//!     child.
//!   - `{ "__keep__": [i, j, …] }` is the path form — walks child `i` of
//!     the matched node, then child `j` of that node, and so on, and
//!     reuses the remapped output of the descendant at that path (the
//!     `KeepPath` analogue — e.g. a `match` arm's body two levels down).
//!   - `{ "__fresh__": "hint" }` mints a unique `var` leaf
//!     (`__fresh_<hint>_<n>`; the same hint reuses the same leaf within
//!     one rule application).
//!
//!   Inside a spec's `payload`, any value position may hold a single-key
//!   marker object `{ "__node__": <spec> }`, `{ "__keep__": n | [..] }`,
//!   or `{ "__fresh__": "hint" }` — realized to the arena index of the
//!   produced node (this is how payload-carried child references like a
//!   `function` node's `body` field are expressed). Plain payload data is
//!   untouched.
//!
//! Realized nodes inherit the matched node's `span` — the engine's
//! documented span policy for rewrites.
//!
//! ## Notes / rough edges
//!
//! - The `.hnl` function cannot observe or produce raw arena indices —
//!   `__keep__` is the only handle onto existing subtrees.
//! - Invocation goes through [`HandlFunction`]'s per-thread cached bridge
//!   `Runtime` (the `pred.hnl`/`hnl-engine-test` pattern): the node
//!   record crosses as an `__arg_0` global and the result returns through
//!   the serde_json boundary, so the function must not return arrow values
//!   or other reserved-key (`__arrow__`/`__tag__`/…) shapes.
//! - The bridge runtime runs its own `desugar` pass when evaluating
//!   `(source).fn(arg)`; since it builds its own canonical `Language`,
//!   user-authored `handl:` rules never fire inside the bridge — no
//!   recursion through the transform engine.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use handl_language::{Ast, AstNode, AstRef, LangError, Language};
use handl_platform::{EvalContext, Function, Module};
use hnl_engine_transform::native::{
    NativeRewrite, RewriteCtx, install_transform_overlay,
};
use serde_json::Value;

use crate::HandlModule;

/// The `TransformRule::Native` name prefix that routes a rule to a
/// `.hnl` function: `"handl:<module>.<fn>"`.
pub const HANDL_PREFIX: &str = "handl:";

/// The marker key a `.hnl` transform returns to reuse a remapped child of
/// the matched node: `{ "__keep__": n }` or `{ "__keep__": [path] }`.
pub const KEEP_KEY: &str = "__keep__";

/// The marker key a `.hnl` transform returns to request a fresh `var` leaf:
/// `{ "__fresh__": "hint" }`.
pub const FRESH_KEY: &str = "__fresh__";

/// The marker key a `.hnl` transform returns *inside a payload* to embed a
/// realized subtree index at an arbitrary payload position:
/// `{ "__node__": <spec> }` (e.g. a `function` node's `body` field).
pub const NODE_KEY: &str = "__node__";

thread_local! {
    /// Reentrancy guard: `true` while a `handl:` rewrite invocation is in
    /// flight on this thread. A nested `handl:` name — e.g. a `match` node
    /// inside the *bridge* runtime's own parse of the module source —
    /// resolves as unprovided so dispatch falls through to the fallback
    /// registry (the Rust natives). This preserves the invariant that
    /// `.hnl` rewrites never recurse through the transform engine.
    static HANDL_REWRITE_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// Whether a `handl:` rewrite is currently in flight on this thread.
pub fn handl_rewrite_active() -> bool {
    HANDL_REWRITE_ACTIVE.get()
}

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

impl HandlNativeRewrite {
    /// Resolve `handl:<module>.<fn>` to the callable, or `None` when the
    /// module/function is not registered (caller decides fallback vs error).
    fn resolve<'a>(&'a self, name: &str) -> Option<&'a (dyn Function + 'static)> {
        let rest = name.strip_prefix(HANDL_PREFIX)?;
        let (module_name, fn_name) = rest.rsplit_once('.')?;
        let module = self.modules.get(module_name)?;
        module.function(fn_name)
    }

    /// Serve `name` from the fallback registry. `handl:` names are tried
    /// verbatim first (a fallback may register the prefixed spelling, e.g.
    /// the desugar registry dual-registers `handl:desugar.*`), then with
    /// the prefix stripped (`desugar.*`).
    fn delegate(&self, name: &str, ctx: &mut RewriteCtx<'_>) -> Result<AstRef, LangError> {
        let Some(f) = &self.fallback else {
            return Err(LangError::engine(format!(
                "unknown native rewrite: {name} (no handl module or fallback registered)"
            )));
        };
        let rest = name.strip_prefix(HANDL_PREFIX).unwrap_or(name);
        if f.provides(name) || !f.provides(rest) {
            f.rewrite(name, ctx)
        } else {
            f.rewrite(rest, ctx)
        }
    }
}

impl NativeRewrite for HandlNativeRewrite {
    fn rewrite(&self, name: &str, ctx: &mut RewriteCtx<'_>) -> Result<AstRef, LangError> {
        if !name.starts_with(HANDL_PREFIX) {
            return self.delegate(name, ctx);
        }
        if HANDL_REWRITE_ACTIVE.get() {
            // Nested `handl:` rewrite (a bridge runtime's own desugar pass
            // hit a `handl:` name while evaluating a rewrite) — serve from
            // the fallback so `.hnl` rewrites never recurse through the
            // transform engine.
            return self.delegate(name, ctx);
        }
        let Some(function) = self.resolve(name) else {
            // A `handl:` name with no registered module/function delegates
            // to the fallback (prefix-stripped), so `handl:desugar.data`
            // still runs the Rust `desugar.data` in hosts that never
            // installed the `.hnl` module.
            return self.delegate(name, ctx);
        };

        // Serialize the matched node (recursively) + the arena env into
        // the value contract.
        let node_value = serialize_node_with_env(ctx.lang(), ctx);

        HANDL_REWRITE_ACTIVE.set(true);
        let result = function
            .call(&[node_value], &mut EvalContext::new())
            .map_err(|e| LangError::engine(format!("handl transform '{name}' failed: {e}")));
        HANDL_REWRITE_ACTIVE.set(false);
        let result = result?;

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

    fn provides(&self, name: &str) -> bool {
        if !name.starts_with(HANDL_PREFIX) {
            return self.fallback.as_ref().is_some_and(|f| f.provides(name));
        }
        if HANDL_REWRITE_ACTIVE.get() {
            return false;
        }
        self.resolve(name).is_some()
    }
}

/// Serialize `node` in `ast` into the `{kind, span, payload, children,
/// roles}` value contract (children recursively). `roles` resolves the
/// node's shape child-roles arity-aware (ADR-0081) so `.hnl` rewrites
/// read child positions by name — `roles.block`, `roles.subject`, … —
/// instead of hardcoding positions.
fn serialize_node_at(lang: &Language, ast: &Ast, node: &AstNode) -> Value {
    let mut payload = node.payload.clone();
    let mut children = Vec::new();
    if let Some(arr) = node.payload.get("children").and_then(|c| c.as_array()) {
        for child_ref in arr {
            if let Some(idx) = child_ref.as_u64() {
                if let Some(child) = ast.get(AstRef::new(idx as usize)) {
                    children.push(serialize_node_at(lang, ast, child));
                }
            }
        }
    }
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("children");
    }
    let mut roles = serde_json::Map::new();
    if let Some(def) = lang.ast.get_by_identity(&node.kind) {
        if let Some(specs) = &def.children {
            for spec in specs {
                if let Ok(Some(i)) = lang.child_role_index_for(node, &spec.role) {
                    roles.insert(spec.role.clone(), Value::from(i));
                }
            }
        }
    }
    serde_json::json!({
        "kind": node.kind,
        "span": node.span.map(|s| serde_json::json!({"start": s.start, "end": s.end})).unwrap_or(Value::Null),
        "payload": payload,
        "children": children,
        "roles": roles,
    })
}

/// The flat arena view carried on the top-level record's `env` field:
/// `{self, nodes:[{index, kind, span, payload}]}` — payloads verbatim
/// (`payload.children` are input-arena indices into `env.nodes`), so a
/// `.hnl` rewrite can navigate beyond its own subtree (declaration
/// lookup, sibling search) without a kind-specific engine service.
fn serialize_env(ctx: &RewriteCtx<'_>) -> Value {
    let lang = ctx.lang();
    let nodes: Vec<Value> = (0..ctx.ast().len())
        .filter_map(|i| {
            ctx.ast().get(AstRef::new(i)).map(|n| {
                let mut roles = serde_json::Map::new();
                if let Some(def) = lang.ast.get_by_identity(&n.kind) {
                    if let Some(specs) = &def.children {
                        for spec in specs {
                            if let Ok(Some(idx)) = lang.child_role_index_for(n, &spec.role) {
                                roles.insert(spec.role.clone(), Value::from(idx));
                            }
                        }
                    }
                }
                serde_json::json!({
                    "index": i,
                    "kind": n.kind,
                    "span": n.span.map(|s| serde_json::json!({"start": s.start, "end": s.end})).unwrap_or(Value::Null),
                    "payload": n.payload,
                    "roles": roles,
                })
            })
        })
        .collect();
    serde_json::json!({
        "self": ctx.node_ref().index(),
        "nodes": nodes,
    })
}

/// Serialize the matched node plus the environment view for a `handl:`
/// call (the top-level record gains `env`; children keep the plain
/// `{kind, span, payload, children, roles}` shape).
fn serialize_node_with_env(lang: &Language, ctx: &RewriteCtx<'_>) -> Value {
    let mut v = serialize_node_at(lang, ctx.ast(), ctx.node());
    if let Some(obj) = v.as_object_mut() {
        obj.insert("env".to_string(), serialize_env(ctx));
    }
    v
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
        if let Some(path) = keep.as_array() {
            // Path form `{__keep__: [i, j, …]}` — walk child positions
            // through the input arena, then remap (`RewriteCtx::remap_path`).
            let path: Vec<usize> = path
                .iter()
                .map(|v| {
                    v.as_u64().map(|i| i as usize).ok_or_else(|| {
                        LangError::engine(format!(
                            "'{KEEP_KEY}' path must contain child indices, got {keep}"
                        ))
                    })
                })
                .collect::<Result<_, _>>()?;
            return ctx.remap_path(&path).ok_or_else(|| {
                LangError::engine(format!(
                    "'{KEEP_KEY}' marker references path {path:?} which has no remapped output"
                ))
            });
        }
        let i = keep.as_u64().ok_or_else(|| {
            LangError::engine(format!(
                "'{KEEP_KEY}' marker must be a child index or path, got {keep}"
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
    payload = realize_value(&payload, ctx, fresh_counter, fresh_names)?;
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

/// Realize marker objects inside a spec's `payload` into arena indices.
///
/// A single-key `{ "__node__": <spec> }`, `{ "__keep__": n | [path] }`, or
/// `{ "__fresh__": "hint" }` value becomes the arena index of the node the
/// spec/marker produces — this is how payload-carried child references
/// (e.g. `function.payload.body`) are expressed. Other values pass through
/// recursively (objects and arrays are walked).
fn realize_value(
    v: &Value,
    ctx: &mut RewriteCtx<'_>,
    fresh_counter: &mut usize,
    fresh_names: &mut HashMap<String, usize>,
) -> Result<Value, LangError> {
    match v {
        Value::Object(map) if map.len() == 1 => {
            if let Some(spec) = map.get(NODE_KEY) {
                return Ok(Value::from(realize(spec, ctx, fresh_counter, fresh_names)?));
            }
            if map.contains_key(KEEP_KEY) || map.contains_key(FRESH_KEY) {
                return Ok(Value::from(realize(v, ctx, fresh_counter, fresh_names)?));
            }
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, val) in map {
                out.insert(k.clone(), realize_value(val, ctx, fresh_counter, fresh_names)?);
            }
            Ok(Value::Object(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, val) in map {
                out.insert(k.clone(), realize_value(val, ctx, fresh_counter, fresh_names)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(items) => Ok(Value::Array(
            items
                .iter()
                .map(|i| realize_value(i, ctx, fresh_counter, fresh_names))
                .collect::<Result<_, _>>()?,
        )),
        _ => Ok(v.clone()),
    }
}

// ---------------------------------------------------------------------------
// The canonical `desugar` overlay (ADR-0094 — desugar as `.hnl` rewrites)
// ---------------------------------------------------------------------------

/// The handl source of `handl-packages/desugar/desugar.hnl` — the
/// `.hnl`-authored port of the `desugar.data`/`desugar.match_recursor`
/// natives (the last non-data-driven transform steps).
pub const DESUGAR_HNL: &str = include_str!("../../../handl-packages/desugar/desugar.hnl");

/// The exported names of the `desugar` overlay module.
///
/// `match_recursor` is deliberately absent: the `.hnl` rewrite contract
/// hands every rewrite the whole serialized arena (`env.nodes`), so a
/// match-heavy AST paid a full arena serialization + conversion per
/// `match` node (74 matches x ~50 ms on the 32k-node typecheck model).
/// The Rust native keeps a once-per-pass constructor index instead, so
/// `handl:desugar.match_recursor` falls through to it via `delegate`.
const DESUGAR_FUNCS: &[&str] = &["data"];

/// Build the `desugar` [`HandlModule`] backing `handl:desugar.*` rewrites.
///
/// `char` is the only dependency (the port tokenizes type expressions on
/// the `char` floor); `bytecode` keeps the per-call module evaluation
/// cheap (the desugar pass fires once per `data`/`match` node in every
/// parse).
pub fn desugar_module() -> HandlModule {
    HandlModule::new("desugar", DESUGAR_HNL, DESUGAR_FUNCS)
        .with_dependency_arc(Arc::new(handl_plugin_lib_char_native::CharModule))
        .with_eval_engine("tree-walk")
}

/// Install the process-wide transform overlay
/// ([`install_transform_overlay`]) resolving `handl:desugar.*` rewrite
/// names against [`desugar_module`]. First install wins; the overlay is
/// consulted by `RewriteEngine::with_desugar_natives` — including the
/// `desugar` pass inside `Runtime::parse` — before the Rust natives, which
/// remain as the bootstrap/fallback layer.
pub fn install_desugar_overlay() {
    let _ = install_transform_overlay(Arc::new(
        HandlNativeRewrite::new().with_module(desugar_module()),
    ));
}
