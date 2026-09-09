//! Handl-authored lint rules (ADR-0097 — the `handl:<mod>.<fn>` spelling,
//! mirroring ADR-0094's `HandlNativeRewrite` for transforms).
//!
//! A `LintRuleDef` can name a `.hnl` function as its check using the
//! [`LintCheck::Native`] dispatch spelling with a `handl:` name prefix:
//!
//! ```yaml
//! - id: lint.my-rule
//!   name: my-rule
//!   severity: warning
//!   message: "my-rule fired on '{name}'"
//!   check: !Native "handl:mymod.myfn"
//! ```
//!
//! `handl:mymod.myfn` resolves to function `myfn` on the [`HandlModule`]
//! registered under module name `mymod` with a [`HandlNativeLint`]
//! adapter. The adapter is a [`NativeLintResolver`], not a name in the
//! map — `handl:` names are dynamic — so a host wires it once onto a
//! [`NativeLintRegistry`] and every `handl:` rule dispatches:
//!
//! ```ignore
//! let mut reg = canonical_lint_natives();
//! reg.add_resolver(
//!     HandlNativeLint::new()
//!         .with_module(HandlModule::new("mymod", MY_HNL, &["myfn"]))
//!         .resolver(),
//! );
//! let engine = LintEngine::new().with_native_registry(reg);
//! ```
//!
//! ## Contract: module-level, once per lint run
//!
//! A `!Native` check is invoked **once per module** — [`LintCtx`] hands
//! Rust natives every parsed statement plus the source text, and this
//! adapter passes the same view through. A *per-node* contract was
//! considered and rejected: `LintCheck::Native` carries no `node_kind`
//! filter, so a per-node `handl:` check could express nothing that
//! `!Match { node_kind, when }` (which already runs the full
//! `TransformCondition` vocabulary per node) cannot. The point of a
//! `.hnl` native is exactly the module-level view — scope analysis,
//! cross-statement conventions — that `!Match` cannot reach.
//!
//! The `.hnl` function is called with **one argument**, a JSON-shaped
//! module record:
//!
//! ```text
//! { source: "<the module source text>",
//!   statements: [
//!     { text: "<statement chunk>",
//!       offset: <byte offset of `text` in `source`>,
//!       root: <node record> },
//!     …
//!   ] }
//! ```
//!
//! `<node record>` is the same shape transforms use —
//! `{kind, span, payload, children}` with `children` recursively
//! serialized — except that every `span` is already **absolute**: each
//! statement's `offset` has been added, so spans read off node records can
//! be reported verbatim in hits.
//!
//! The function returns either:
//!
//! - `null` or `[]` — no violations.
//! - a **list of hit records**:
//!
//! ```text
//! { name: "x",                        -- optional; fills the {name}
//!                                     --   placeholder of the rule's
//!                                     --   message template
//!   args: { "<key>": "<value>", … },  -- optional; extra {key} args
//!   span: { start: <byte>, end: <byte> },  -- optional; absolute
//!   fix: { start: <byte>, end: <byte>,
//!          replacement: "<text>" }    -- optional; a LintFixOut
//! }
//! ```
//!
//! (`name` is sugar for `args.name`; an explicit `args.name` wins.)
//!
//! ## Notes / rough edges
//!
//! - Invocation goes through [`HandlFunction`]'s per-thread cached bridge
//!   `Runtime`, exactly like [`HandlNativeRewrite`] — the module record
//!   crosses as an `__arg_0` global and hits return through the serde_json
//!   boundary, so the function must not return arrow values or other
//!   reserved-key (`__arrow__`/`__tag__`/…) shapes.
//! - Like `HandlNativeRewrite`, nothing wires this adapter in the shipped
//!   hosts yet — it is the opt-in seam a host (or `lints.yaml` third-party
//!   language) uses when it wants `.hnl`-authored lint rules.
//!
//! [`LintCheck::Native`]: handl_language::LintCheck::Native
//! [`HandlFunction`]: crate::HandlFunction
//! [`HandlNativeRewrite`]: crate::HandlNativeRewrite
//! [`NativeLintRegistry`]: handl_plugin_engine_lint::NativeLintRegistry

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use handl_language::{Ast, AstNode, AstRef, LangError, Span};
use handl_platform::{EvalContext, Module};
use handl_plugin_engine_lint::{LintCtx, LintFixOut, NativeHit, NativeLintFn, NativeLintResolver};
use serde_json::Value;

use crate::HandlModule;
use crate::transform::HANDL_PREFIX;

/// A [`NativeLintResolver`] that routes `handl:<module>.<fn>` lint rule
/// names to functions on registered [`HandlModule`]s — the lint analogue
/// of [`HandlNativeRewrite`](crate::HandlNativeRewrite).
///
/// Non-`handl:` names are declined (`None`), so map entries and earlier
/// resolvers keep resolving; wire it with
/// `NativeLintRegistry::add_resolver`.
#[derive(Clone, Default)]
pub struct HandlNativeLint {
    /// Module name → module providing the lint functions.
    modules: HashMap<String, Arc<HandlModule>>,
}

impl HandlNativeLint {
    /// Create an empty adapter (no modules).
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a [`HandlModule`] under its module name. Functions are
    /// resolved lazily at lint time (`module.function(fn_name)`).
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

    /// The [`NativeLintResolver`] to register on a `NativeLintRegistry`.
    /// Cheap to clone — the underlying module map is `Arc`ed.
    pub fn resolver(&self) -> NativeLintResolver {
        let modules = self.modules.clone();
        Arc::new(move |name: &str| resolve(&modules, name))
    }
}

/// Resolve one `!Native` name: `None` for non-`handl:` names, `Some` of a
/// lint fn (possibly one that reports the resolution error at call time)
/// for `handl:` names.
fn resolve(modules: &HashMap<String, Arc<HandlModule>>, name: &str) -> Option<NativeLintFn> {
    let rest = name.strip_prefix(HANDL_PREFIX)?;

    let err_fn =
        |msg: String| -> NativeLintFn { Arc::new(move |_| Err(LangError::engine(msg.clone()))) };

    let Some((module_name, fn_name)) = rest.rsplit_once('.') else {
        return Some(err_fn(format!(
            "handl lint name '{name}' must be 'handl:<module>.<fn>'"
        )));
    };
    let Some(module) = modules.get(module_name) else {
        return Some(err_fn(format!(
            "handl lint '{name}': no HandlModule registered under '{module_name}'"
        )));
    };

    let module = Arc::clone(module);
    let module_name = module_name.to_string();
    let fn_name = fn_name.to_string();
    let rule = name.to_string();
    Some(Arc::new(move |ctx: &LintCtx<'_>| {
        let function = module.function(&fn_name).ok_or_else(|| {
            LangError::engine(format!(
                "handl lint '{rule}': module '{module_name}' has no function '{fn_name}'"
            ))
        })?;
        let module_value = serialize_module(ctx);
        let result = function
            .call(&[module_value], &mut EvalContext::new())
            .map_err(|e| LangError::engine(format!("handl lint '{rule}' failed: {e}")))?;
        hits_from_value(&rule, &result)
    }))
}

/// Serialize the [`LintCtx`] module view into the `{source, statements}`
/// record contract. Node spans are absolutized (`+ stmt.offset`) so the
/// `.hnl` function can report them verbatim.
fn serialize_module(ctx: &LintCtx<'_>) -> Value {
    let statements: Vec<Value> = ctx
        .module
        .iter()
        .map(|s| {
            serde_json::json!({
                "text": s.text,
                "offset": s.offset,
                "root": s.ast.get(s.root).map(|n| serialize_node(&s.ast, n, s.offset)).unwrap_or(Value::Null),
            })
        })
        .collect();
    serde_json::json!({
        "source": ctx.src,
        "statements": statements,
    })
}

/// Serialize `node` (and its children, recursively) into the
/// `{kind, span, payload, children}` node record, with `span` as absolute
/// byte offsets (`offset` is the owning statement's offset in the source).
fn serialize_node(ast: &Ast, node: &AstNode, offset: usize) -> Value {
    let mut payload = node.payload.clone();
    let mut children = Vec::new();
    if let Some(arr) = node.payload.get("children").and_then(|c| c.as_array()) {
        for child_ref in arr {
            if let Some(idx) = child_ref.as_u64() {
                if let Some(child) = ast.get(AstRef::new(idx as usize)) {
                    children.push(serialize_node(ast, child, offset));
                }
            }
        }
    }
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("children");
    }
    serde_json::json!({
        "kind": node.kind,
        "span": node.span.map(|s| serde_json::json!({"start": s.start + offset, "end": s.end + offset})).unwrap_or(Value::Null),
        "payload": payload,
        "children": children,
    })
}

/// Map the function's return value to [`NativeHit`]s: `null`/`[]` → none;
/// otherwise a list of hit records.
fn hits_from_value(rule: &str, result: &Value) -> Result<Vec<NativeHit>, LangError> {
    if result.is_null() {
        return Ok(Vec::new());
    }
    let arr = result.as_array().ok_or_else(|| {
        LangError::engine(format!(
            "handl lint '{rule}' must return null or a list of hit records; got {result}"
        ))
    })?;
    arr.iter().map(|v| hit_from_value(rule, v)).collect()
}

/// Map one hit record `{name?, args?, span?, fix?}` to a [`NativeHit`].
fn hit_from_value(rule: &str, v: &Value) -> Result<NativeHit, LangError> {
    let obj = v.as_object().ok_or_else(|| {
        LangError::engine(format!(
            "handl lint '{rule}': hit records must be objects; got {v}"
        ))
    })?;

    let mut args = BTreeMap::new();
    if let Some(name) = obj.get("name") {
        let name = name.as_str().ok_or_else(|| {
            LangError::engine(format!(
                "handl lint '{rule}': hit 'name' must be a string; got {name}"
            ))
        })?;
        args.insert("name".to_string(), name.to_string());
    }
    if let Some(extra) = obj.get("args") {
        let extra = extra.as_object().ok_or_else(|| {
            LangError::engine(format!(
                "handl lint '{rule}': hit 'args' must be a record; got {extra}"
            ))
        })?;
        for (k, val) in extra {
            let s = val.as_str().ok_or_else(|| {
                LangError::engine(format!(
                    "handl lint '{rule}': hit 'args.{k}' must be a string; got {val}"
                ))
            })?;
            args.insert(k.clone(), s.to_string());
        }
    }

    let span = match obj.get("span") {
        None | Some(Value::Null) => None,
        Some(s) => Some(span_from_value(rule, s, "span")?),
    };

    let fix = match obj.get("fix") {
        None | Some(Value::Null) => None,
        Some(f) => {
            let fobj = f.as_object().ok_or_else(|| {
                LangError::engine(format!(
                    "handl lint '{rule}': hit 'fix' must be a record; got {f}"
                ))
            })?;
            let span = span_from_value(rule, f, "fix")?;
            let replacement = fobj
                .get("replacement")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    LangError::engine(format!(
                        "handl lint '{rule}': hit 'fix' requires a string 'replacement' field"
                    ))
                })?
                .to_string();
            Some(LintFixOut {
                start: span.start,
                end: span.end,
                replacement,
            })
        }
    };

    Ok(NativeHit { args, span, fix })
}

/// Read `{start, end}` byte offsets out of a span-ish record.
fn span_from_value(rule: &str, v: &Value, what: &str) -> Result<Span, LangError> {
    let get = |key: &str| -> Result<usize, LangError> {
        v.get(key)
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .ok_or_else(|| {
                LangError::engine(format!(
                    "handl lint '{rule}': hit '{what}' requires a '{key}' byte offset; got {v}"
                ))
            })
    };
    Ok(Span {
        start: get("start")?,
        end: get("end")?,
    })
}
