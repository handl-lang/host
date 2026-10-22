//! VM primitives module — host functions for the exec model's VM instruction set.
//!
//! `exec_model.hnl` (ADR-0103) defines structural ops that orchestrate VM
//! primitives (`push_scope`, `pop_scope`, `bind`, `lookup`, `call`, `ffi`,
//! `make_closure`, `pattern_match`, `lookup_eval_scheme`, `setfield`, `ctx`).
//! These are the irreducible machine — the hardware boundary / VM instruction
//! set. On the tree-walk runtime they are not native ops; they are host
//! functions registered via this module so the exec model's structural ops
//! become executable for testing.
//!
//! The implementations are minimal — they handle the common cases needed by
//! the exec model parity tests. They are not production-grade:
//!
//! - `lookup_eval_scheme` returns a host-function name string for simple kinds
//!   (`Arr`, `Const`); the corresponding `eval_scheme_*` host functions extract
//!   the `value` field directly. Kinds requiring recursive evaluation (e.g.
//!   `Seq`, `Tag`) are not supported — the eval scheme host function would need
//!   to call back into the exec model's `eval`, which is a local binding not
//!   accessible from a host function.
//! - `make_closure` creates a closure record (`{"__closure__": true, ...}`);
//!   `call` returns the `body` field for closure records (cannot evaluate the
//!   body from a host function). Arrow-values are dispatched via
//!   `ctx.try_call_value`.
//! - `vm_pattern_match` handles wildcard (`"_"`), variable (string pattern),
//!   and literal equality. Structured patterns (records, arrays, tag patterns)
//!   are not supported.
//! - `ffi` handles a few native arithmetic ops (`add`, `mul`, `sub`, `div`);
//!   other dispatches return `Null`.
//! - `ctx` / `setfield` manage a simple in-memory context record with
//!   `handler_stack` and `exports` fields.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_value::{TaggedValue, sym_from_parent_value, tagged_to_value};
use serde_json::{Map, Value, json};

// ---------------------------------------------------------------------------
// VmState — shared mutable state behind a Mutex (Send + Sync)
// ---------------------------------------------------------------------------

/// The mutable state of the VM primitives module: a scope stack and a context
/// record.
struct VmState {
    /// The scope stack — a stack of name→value maps. `push_scope` pushes a
    /// fresh map; `bind` inserts into the top map; `lookup` searches
    /// innermost-first.
    scopes: Vec<HashMap<String, Value>>,
    /// The VM context record, exposed via the `ctx()` host function and
    /// mutated via `setfield`. Contains `handler_stack` and `exports`.
    context: Value,
}

impl VmState {
    fn new() -> Self {
        VmState {
            scopes: Vec::new(),
            context: json!({
                "handler_stack": [],
                "exports": {}
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Function structs — each holds a clone of the shared state Arc
// ---------------------------------------------------------------------------

/// `push_scope()` — push a fresh scope onto the scope stack.
struct PushScopeFn {
    state: Arc<Mutex<VmState>>,
}

impl Function for PushScopeFn {
    fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let mut s = self.state.lock().unwrap();
        s.scopes.push(HashMap::new());
        Ok(Value::Null)
    }
}

/// `pop_scope()` — pop the top scope from the scope stack.
struct PopScopeFn {
    state: Arc<Mutex<VmState>>,
}

impl Function for PopScopeFn {
    fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let mut s = self.state.lock().unwrap();
        s.scopes.pop();
        Ok(Value::Null)
    }
}

/// `bind(name, value)` — bind `name` → `value` in the current (top) scope.
struct BindFn {
    state: Arc<Mutex<VmState>>,
}

impl Function for BindFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let name = args
            .get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| EvalError::Other("bind: expected name string".to_string()))?;
        let value = args.get(1).cloned().unwrap_or(Value::Null);
        let mut s = self.state.lock().unwrap();
        if s.scopes.is_empty() {
            s.scopes.push(HashMap::new());
        }
        let top = s.scopes.last_mut().unwrap();
        top.insert(name.to_string(), value.clone());
        Ok(value)
    }
}

/// `lookup(name)` — look up `name` in the scope stack (innermost-first).
/// Returns the bound value or `Null` if unbound.
struct LookupFn {
    state: Arc<Mutex<VmState>>,
}

impl Function for LookupFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let name = args
            .get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| EvalError::Other("lookup: expected name string".to_string()))?;
        let s = self.state.lock().unwrap();
        for scope in s.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Ok(val.clone());
            }
        }
        Ok(Value::Null)
    }
}

/// `make_closure(params, body, env)` — create a closure record.
struct MakeClosureFn;

impl Function for MakeClosureFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let params = args.get(0).cloned().unwrap_or(Value::Null);
        let body = args.get(1).cloned().unwrap_or(Value::Null);
        let env = args.get(2).cloned().unwrap_or(Value::Null);
        Ok(json!({
            "__closure__": true,
            "params": params,
            "body": body,
            "env": env
        }))
    }
}

/// `call(callee, args)` — apply `callee` to `args`.
///
/// - Arrow-values (`{"__arrow__": true, ...}`) are dispatched via
///   `ctx.try_call_value`.
/// - Closure records (`{"__closure__": true, ...}`) return the `body` field
///   (minimal — cannot evaluate the body from a host function).
/// - Other callees return `Null`.
struct CallFn;

impl Function for CallFn {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let callee = args.get(0).cloned().unwrap_or(Value::Null);
        let call_args: Vec<Value> = args
            .get(1)
            .and_then(|v| v.as_array())
            .map(|a| a.clone())
            .unwrap_or_default();

        // Arrow-value: dispatch via the tree-walk engine's ValueCallable.
        if callee
            .as_object()
            .and_then(|o| o.get("__arrow__"))
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            if let Some(result) = ctx.try_call_value(&callee, &call_args) {
                return result;
            }
            return Ok(Value::Null);
        }

        // Closure record: return the body field (minimal).
        if callee
            .as_object()
            .and_then(|o| o.get("__closure__"))
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            return Ok(callee.get("body").cloned().unwrap_or(Value::Null));
        }

        Ok(Value::Null)
    }
}

/// `vm_pattern_match(pattern, value)` — match `value` against `pattern`.
///
/// - Wildcard `"_"` → `{}` (match success, no bindings).
/// - Variable (any other string) → `{pattern: value}` (bind the variable).
/// - Literal equality → `{}` (match success).
/// - Otherwise → `Null` (no match).
struct VmPatternMatchFn;

impl Function for VmPatternMatchFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let pattern = args.get(0).cloned().unwrap_or(Value::Null);
        let value = args.get(1).cloned().unwrap_or(Value::Null);

        // Wildcard: matches anything.
        if pattern.as_str() == Some("_") {
            return Ok(json!({}));
        }

        // Variable pattern: a non-literal string binds to the value.
        if let Some(name) = pattern.as_str() {
            let mut bindings = Map::new();
            bindings.insert(name.to_string(), value);
            return Ok(Value::Object(bindings));
        }

        // Literal equality.
        if pattern == value {
            return Ok(json!({}));
        }

        // No match.
        Ok(Value::Null)
    }
}

/// `lookup_eval_scheme(kind)` — return the eval scheme name for a node kind.
///
/// Returns `"eval_scheme_<kind>"` for known kinds (`Arr`, `Const`), or `Null`
/// for unknown kinds.
struct LookupEvalSchemeFn;

impl Function for LookupEvalSchemeFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let kind = args.get(0).and_then(|v| v.as_str()).ok_or_else(|| {
            EvalError::Other("lookup_eval_scheme: expected kind string".to_string())
        })?;
        match kind {
            "Arr" | "Const" => Ok(Value::String(format!("eval_scheme_{kind}"))),
            _ => Ok(Value::Null),
        }
    }
}

/// `eval_scheme_Arr(node)` — extract the `value` field from an Arr node.
struct EvalSchemeArrFn;

impl Function for EvalSchemeArrFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let node = args.get(0).cloned().unwrap_or(Value::Null);
        Ok(node.get("value").cloned().unwrap_or(Value::Null))
    }
}

/// `eval_scheme_Const(node)` — extract the `value` field from a Const node.
struct EvalSchemeConstFn;

impl Function for EvalSchemeConstFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let node = args.get(0).cloned().unwrap_or(Value::Null);
        Ok(node.get("value").cloned().unwrap_or(Value::Null))
    }
}

/// `ffi(module, name, args)` — foreign function interface (minimal).
///
/// Handles `ffi("native", "add", [a, b])` and a few other arithmetic ops.
/// Other dispatches return `Null`.
struct FfiFn;

impl Function for FfiFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let module = args.get(0).and_then(|v| v.as_str()).unwrap_or("");
        let name = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
        let ffi_args = args.get(2).and_then(|v| v.as_array());

        if module == "native" {
            if let Some(arr) = ffi_args {
                let a = arr.get(0).and_then(|v| v.as_i64());
                let b = arr.get(1).and_then(|v| v.as_i64());
                match name {
                    "add" if a.is_some() && b.is_some() => {
                        return Ok(json!(a.unwrap() + b.unwrap()));
                    }
                    "sub" if a.is_some() && b.is_some() => {
                        return Ok(json!(a.unwrap() - b.unwrap()));
                    }
                    "mul" if a.is_some() && b.is_some() => {
                        return Ok(json!(a.unwrap() * b.unwrap()));
                    }
                    "div" if a.is_some() && b.is_some() && b.unwrap() != 0 => {
                        return Ok(json!(a.unwrap() / b.unwrap()));
                    }
                    _ => {}
                }
            }
        }

        // module load: return empty exports.
        if module == "module" && name == "load" {
            return Ok(json!({}));
        }

        Ok(Value::Null)
    }
}

/// `setfield(obj, key, value)` — set a field on the VM context.
///
/// Updates the internal context's `key` field to `value` and returns `value`.
/// The first argument (the context object) is used for identification but the
/// actual update is to the internal mutable state.
struct SetFieldFn {
    state: Arc<Mutex<VmState>>,
}

impl Function for SetFieldFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let key = args
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| EvalError::Other("setfield: expected key string".to_string()))?;
        let value = args.get(2).cloned().unwrap_or(Value::Null);

        let mut s = self.state.lock().unwrap();
        if let Some(obj) = s.context.as_object_mut() {
            obj.insert(key.to_string(), value.clone());
        }
        Ok(value)
    }
}

/// `ctx()` — return the current VM context as a JSON object.
struct CtxFn {
    state: Arc<Mutex<VmState>>,
}

impl Function for CtxFn {
    fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let s = self.state.lock().unwrap();
        Ok(s.context.clone())
    }
}

/// `eval(node)` — evaluate a node by dispatching on its `kind` field.
///
/// This host function exists because `exec_model.hnl` defines `eval` near
/// the end of its `do { ... }` block (after `eval_block`, `proj_child`,
/// etc.). The block uses sequential binding, so functions defined earlier
/// cannot capture `eval` in their closure environment. When those earlier
/// functions call `eval(...)`, the tree-walk engine falls back to
/// `call_host_fn("eval", ...)`, which dispatches here.
///
/// Minimal: handles `Arr` and `Const` kinds (returns `node.value`).
/// Other kinds return `Null`.
struct EvalFn;

impl Function for EvalFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let node = args.get(0).cloned().unwrap_or(Value::Null);
        let kind = node.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        match kind {
            "Arr" | "Const" => Ok(node.get("value").cloned().unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }
}

/// `find_op_handler(stack, name)` — search the handler stack for an op.
///
/// This host function exists because `exec_model.hnl` defines
/// `find_op_handler` after `call_op` in the `do { ... }` block, so
/// `call_op`'s captured env doesn't include it.
///
/// Walks `stack` (an array) from first to last. For each handler, checks
/// if its `impls` field (an object) has the key `name`. Returns the first
/// matching implementation value, or `Null` if none found.
struct FindOpHandlerFn;

impl Function for FindOpHandlerFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let stack = args.get(0).cloned().unwrap_or(Value::Null);
        let name = args
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| EvalError::Other("find_op_handler: expected name string".to_string()))?;

        if let Some(arr) = stack.as_array() {
            for handler in arr {
                if let Some(impls) = handler.get("impls").and_then(|v| v.as_object()) {
                    if let Some(impl_val) = impls.get(name) {
                        return Ok(impl_val.clone());
                    }
                }
            }
        }
        Ok(Value::Null)
    }
}

/// `array_drop_last(xs)` — return `xs` without its last element.
///
/// This host function exists because `exec_model.hnl` defines
/// `array_drop_last` after `pop_handler` in the `do { ... }` block, so
/// `pop_handler`'s captured env doesn't include it.
///
/// Returns `array.slice(xs, 0, len-1)`. For an empty array, returns `[]`.
struct ArrayDropLastFn;

impl Function for ArrayDropLastFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let xs = args.get(0).cloned().unwrap_or(Value::Null);
        if let Some(arr) = xs.as_array() {
            if arr.is_empty() {
                return Ok(json!([]));
            }
            return Ok(Value::Array(arr[..arr.len() - 1].to_vec()));
        }
        Ok(json!([]))
    }
}

// ---------------------------------------------------------------------------
// Nominal identity primitives (ADR-0106 §2 — Phase 2 kernel)
// ---------------------------------------------------------------------------

/// `sym()` / `sym(parent)` / `sym("seed")` / `sym("seed", parent[, ty])`
/// — nominal identity mints (ADR-0106 §2, ADR-0154 §1/§2).
///
/// - `sym()` — fresh nominal identity with no parent (a root sym).
/// - `sym(A)` — fresh nominal identity whose nominal parent is `A`; `A`
///   must itself be a nominal object (`{"__sym__": ...}`), otherwise a
///   `TypeMismatch` error is returned.
/// - `sym("seed")` — the *deterministic* mint of ADR-0154 §1: the
///   nominal for a declared name, idempotent and stable across
///   processes (the handle is a content hash of the seed, not the fresh
///   counter). `sym("type.Int")` twice is the *same* nominal; plain
///   `sym()` stays fresh for locals.
/// - `sym("seed", parent)` / `sym("seed", parent, ty)` — the declared
///   nominal with its links: `parent` is the `<:` edge, `ty` the
///   `type`/membership edge (`Int.ty = Type`; `null` = absent).
///
/// The returned value is the explicit JSON encoding
/// `{"__sym__": {"handle": <n>, "name": "...", "parent": <sym or null>,
/// "type": <sym or null>}}`. Identity is the `handle`: two distinct
/// `sym()` calls are never equal; two `sym("seed")` mints are always
/// equal.
///
/// The implementation delegates to `handl-value`, the single owner of the
/// nominal value model and its serialization (ADR-0106 "Implementation
/// notes": `crates/handl-value` — the value model; objects gain a nominal
/// identity field while remaining serializable tagged data).
struct SymFn;

impl Function for SymFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        // Default display name — presentation only, never part of identity.
        const DEFAULT_NAME: &str = "sym";
        let tv = match args.len() {
            0 => handl_value::sym(DEFAULT_NAME),
            // `sym(parent)` — fresh child; `sym("seed")` — seeded root.
            1 => match &args[0] {
                Value::String(seed) => handl_value::sym_seeded(seed),
                other => {
                    sym_from_parent_value(DEFAULT_NAME, &TaggedValue::from(other.clone()))
                        .map_err(EvalError::TypeMismatch)?
                }
            },
            // `sym("seed", parent[, ty])` — the declared mint carrying
            // the `parent` (<:) and `type` (membership) links.
            2 | 3 => {
                let Value::String(seed) = &args[0] else {
                    return Err(EvalError::Other(format!(
                        "sym: expected sym() / sym(parent) / sym(\"seed\"[, parent[, type]]); \
                         {}-argument form takes a seed string first",
                        args.len()
                    )));
                };
                let parent = TaggedValue::from(args[1].clone());
                let ty = args
                    .get(2)
                    .map(|v| TaggedValue::from(v.clone()))
                    .unwrap_or(TaggedValue::Null);
                handl_value::sym_seeded_with_links(seed, &parent, &ty)
                    .map_err(EvalError::TypeMismatch)?
            }
            n => {
                return Err(EvalError::Other(format!(
                    "sym: expected 0–3 arguments \
                     (sym() / sym(parent) / sym(\"seed\"[, parent[, type]])), got {}",
                    n
                )));
            }
        };
        Ok(tagged_to_value(&tv))
    }
}

/// `instance_of(value, nominal)` — instanceof over the nominal links
/// (ADR-0106 §2, reread per ADR-0154 §2).
///
/// The kernel (`handl_value::instance_of`) owns the semantics: `x : T`
/// resolves `x`'s `type`-link head, then the `<:` walk is
/// `head.has_ancestor(nominal)` over `parent` links.
///
/// - `value` a nominal reads both its own `parent` chain (the `<:` read
///   — `Float instanceof Number`) and its `ty` link (the membership read
///   — `Int instanceof Type`).
/// - `value` an object reads its `"type"` field first (ctor values),
///   then the `__tag__` bridge: the datum's seeded nominal's real
///   parent walk, or — for unseeded hand-minted nominals — the legacy
///   presentation-name role match.
/// - `value` a scalar resolves through the literal defaults
///   (`1 instanceof Nat`, `1.5 instanceof Float`, ADR-0154 §5).
/// - `nominal` resolves through `nominal_head` — a sym, a
///   `{"__former__": s}` family object, or a `{"__type__": i}` universe
///   encoding (→ `type.Type`); anything else is a `TypeMismatch`.
/// - A value with no resolvable head is simply not an instance:
///   `Bool(false)`, no error.
struct InstanceOfFn;

impl Function for InstanceOfFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let value = args.get(0).cloned().unwrap_or(Value::Null);
        let nominal = args.get(1).cloned().ok_or_else(|| {
            EvalError::Other("instance_of: expected two arguments (value, nominal)".to_string())
        })?;
        let nominal_tv = TaggedValue::from(nominal.clone());
        if handl_value::nominal_head(&nominal_tv).is_none() {
            return Err(EvalError::TypeMismatch(
                "instance_of: second argument must be a nominal object (see ADR-0106 §2)"
                    .to_string(),
            ));
        }
        // The kernel owns the read (ADR-0154 §2): `x : T` is the `type`
        // link's head, `<:` walks `parent`; nominal values read both
        // links; `__tag__` data bridge through the seeded walk or the
        // legacy name-role match; scalars through the literal defaults.
        Ok(Value::Bool(handl_value::instance_of(
            &TaggedValue::from(value.clone()),
            &nominal_tv,
        )))
    }
}

// The Phase 4 legacy-tag bridge helpers (`legacy_tag_roles` /
// `legacy_tag_instance_of`) moved into `handl-value` with WS-E —
// `handl_value::instance_of` owns both the seeded walk and the
// name-role fallback in one place.
// ---------------------------------------------------------------------------
// VmPrimitivesModule — the Module that registers all VM primitive host fns
// ---------------------------------------------------------------------------

/// A host-function module providing the VM primitives that `exec_model.hnl`
/// calls. Register it as a dependency of the exec model `HandlModule`:
///
/// ```ignore
/// HandlModule::new("exec_model", source, funcs)
///     .with_dependency(VmPrimitivesModule::new())
/// ```
///
/// Each call to [`VmPrimitivesModule::new`] creates a fresh module with its
/// own scope stack and context — state is not shared across instances.
pub struct VmPrimitivesModule {
    push_scope_fn: PushScopeFn,
    pop_scope_fn: PopScopeFn,
    bind_fn: BindFn,
    lookup_fn: LookupFn,
    make_closure_fn: MakeClosureFn,
    call_fn: CallFn,
    vm_pattern_match_fn: VmPatternMatchFn,
    lookup_eval_scheme_fn: LookupEvalSchemeFn,
    eval_scheme_arr_fn: EvalSchemeArrFn,
    eval_scheme_const_fn: EvalSchemeConstFn,
    ffi_fn: FfiFn,
    setfield_fn: SetFieldFn,
    ctx_fn: CtxFn,
    eval_fn: EvalFn,
    find_op_handler_fn: FindOpHandlerFn,
    array_drop_last_fn: ArrayDropLastFn,
    sym_fn: SymFn,
    instance_of_fn: InstanceOfFn,
}

impl VmPrimitivesModule {
    /// Create a fresh `VmPrimitivesModule` with an empty scope stack and a
    /// default context (`{"handler_stack": [], "exports": {}}`).
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(VmState::new()));
        VmPrimitivesModule {
            push_scope_fn: PushScopeFn {
                state: Arc::clone(&state),
            },
            pop_scope_fn: PopScopeFn {
                state: Arc::clone(&state),
            },
            bind_fn: BindFn {
                state: Arc::clone(&state),
            },
            lookup_fn: LookupFn {
                state: Arc::clone(&state),
            },
            make_closure_fn: MakeClosureFn,
            call_fn: CallFn,
            vm_pattern_match_fn: VmPatternMatchFn,
            lookup_eval_scheme_fn: LookupEvalSchemeFn,
            eval_scheme_arr_fn: EvalSchemeArrFn,
            eval_scheme_const_fn: EvalSchemeConstFn,
            ffi_fn: FfiFn,
            setfield_fn: SetFieldFn {
                state: Arc::clone(&state),
            },
            ctx_fn: CtxFn {
                state: Arc::clone(&state),
            },
            eval_fn: EvalFn,
            find_op_handler_fn: FindOpHandlerFn,
            array_drop_last_fn: ArrayDropLastFn,
            sym_fn: SymFn,
            instance_of_fn: InstanceOfFn,
        }
    }
}

impl Default for VmPrimitivesModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VmPrimitivesModule {
    fn name(&self) -> &str {
        "vm_primitives"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "push_scope" => Some(&self.push_scope_fn as &dyn Function),
            "pop_scope" => Some(&self.pop_scope_fn as &dyn Function),
            "bind" => Some(&self.bind_fn as &dyn Function),
            "lookup" => Some(&self.lookup_fn as &dyn Function),
            "make_closure" => Some(&self.make_closure_fn as &dyn Function),
            "call" => Some(&self.call_fn as &dyn Function),
            "vm_pattern_match" => Some(&self.vm_pattern_match_fn as &dyn Function),
            "lookup_eval_scheme" => Some(&self.lookup_eval_scheme_fn as &dyn Function),
            "eval_scheme_Arr" => Some(&self.eval_scheme_arr_fn as &dyn Function),
            "eval_scheme_Const" => Some(&self.eval_scheme_const_fn as &dyn Function),
            "ffi" => Some(&self.ffi_fn as &dyn Function),
            "setfield" => Some(&self.setfield_fn as &dyn Function),
            "ctx" => Some(&self.ctx_fn as &dyn Function),
            "eval" => Some(&self.eval_fn as &dyn Function),
            "find_op_handler" => Some(&self.find_op_handler_fn as &dyn Function),
            "array_drop_last" => Some(&self.array_drop_last_fn as &dyn Function),
            "sym" => Some(&self.sym_fn as &dyn Function),
            "instance_of" => Some(&self.instance_of_fn as &dyn Function),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![
            ("push_scope", &self.push_scope_fn as &dyn Function),
            ("pop_scope", &self.pop_scope_fn as &dyn Function),
            ("bind", &self.bind_fn as &dyn Function),
            ("lookup", &self.lookup_fn as &dyn Function),
            ("make_closure", &self.make_closure_fn as &dyn Function),
            ("call", &self.call_fn as &dyn Function),
            (
                "vm_pattern_match",
                &self.vm_pattern_match_fn as &dyn Function,
            ),
            (
                "lookup_eval_scheme",
                &self.lookup_eval_scheme_fn as &dyn Function,
            ),
            ("eval_scheme_Arr", &self.eval_scheme_arr_fn as &dyn Function),
            (
                "eval_scheme_Const",
                &self.eval_scheme_const_fn as &dyn Function,
            ),
            ("ffi", &self.ffi_fn as &dyn Function),
            ("setfield", &self.setfield_fn as &dyn Function),
            ("ctx", &self.ctx_fn as &dyn Function),
            ("eval", &self.eval_fn as &dyn Function),
            ("find_op_handler", &self.find_op_handler_fn as &dyn Function),
            ("array_drop_last", &self.array_drop_last_fn as &dyn Function),
            ("sym", &self.sym_fn as &dyn Function),
            ("instance_of", &self.instance_of_fn as &dyn Function),
        ]
    }
}

// ---------------------------------------------------------------------------
// SymModule — the `sym` module: ADR-0106 §2 nominal identity primitives
// exposed to Handl surface code (Phase 3).
// ---------------------------------------------------------------------------
//
// `sym()` / `sym(parent)` desugar to native arrow/object structures only:
// a bare `sym(...)` call in surface code parses (via the existing grammar's
// `call` rule — no lexer/parser changes) and dispatches through the module
// registry's flat-name lookup to the Phase 2 VM primitives. `sym.instanceof`
// is a `field-access(var("sym"), "instanceof")` host call resolving to the
// `instance_of` primitive aliased under a surface-friendly local name.
//
// No new closure/capture semantics are introduced: both functions are plain
// host functions delegating to `handl-value` via `VmPrimitivesModule`.

/// Name of the module: `sym(x)` / `sym.instanceof(v, n)` in surface code.
pub const SYM_MODULE_NAME: &str = "sym";

/// A `Send + Sync` wrapper exposing the Phase 2 nominal primitives as the
/// `sym` module: `sym` (0- or 1-arg mint) and `instanceof` (alias of
/// `instance_of`). Delegates to a [`VmPrimitivesModule`] instance held by
/// value (its function structs are `Send + Sync`; shared scope state is
/// behind `Arc<Mutex<..>>`).
pub struct SymModule {
    inner: VmPrimitivesModule,
}

impl SymModule {
    /// Create a fresh `sym` module (own VM primitive state).
    pub fn new() -> Self {
        SymModule {
            inner: VmPrimitivesModule::new(),
        }
    }
}

impl Default for SymModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SymModule {
    fn name(&self) -> &str {
        SYM_MODULE_NAME
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "sym" | "seeded" => self.inner.function("sym"),
            "instanceof" => self.inner.function("instance_of"),
            "instance_of" => self.inner.function("instance_of"),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        let mut out = Vec::new();
        if let Some(f) = self.inner.function("sym") {
            out.push(("sym", f));
            // `seeded` is the explicit spelling of `sym("seed")`
            // (ADR-0154 §1) — same function, arity distinguishes.
            out.push(("seeded", f));
        }
        if let Some(f) = self.inner.function("instance_of") {
            out.push(("instanceof", f));
            out.push(("instance_of", f));
        }
        out
    }
}

#[cfg(test)]
mod sym_module_tests {
    use super::*;

    fn call(module: &SymModule, name: &str, args: &[Value]) -> Result<Value, EvalError> {
        let func = module
            .function(name)
            .unwrap_or_else(|| panic!("SymModule has no '{}' function", name));
        func.call(args, &mut EvalContext::new())
    }

    #[test]
    fn sym_module_name_is_sym() {
        assert_eq!(SymModule::new().name(), "sym");
    }

    #[test]
    fn sym_module_exposes_sym_and_instanceof_aliases() {
        let m = SymModule::new();
        assert!(m.function("sym").is_some());
        assert!(m.function("instanceof").is_some());
        assert!(m.function("instance_of").is_some());
        assert!(m.function("nope").is_none());
    }

    #[test]
    fn sym_module_sym_mints_nominal_and_instanceof_walks_chain() {
        let m = SymModule::new();
        let root = call(&m, "sym", &[]).expect("sym() mints");
        assert!(root.get("__sym__").is_some());
        let child = call(&m, "sym", &[root.clone()]).expect("sym(parent) mints");
        assert_eq!(
            call(&m, "instanceof", &[child.clone(), root.clone()]),
            Ok(json!(true))
        );
        assert_eq!(
            call(&m, "instance_of", &[child.clone(), child.clone()]),
            Ok(json!(true))
        );
        assert_eq!(
            call(&m, "instanceof", &[root.clone(), child.clone()]),
            Ok(json!(false))
        );
    }

    #[test]
    fn sym_module_non_nominal_parent_errors_cleanly() {
        let m = SymModule::new();
        let err = call(&m, "sym", &[json!(42)]).unwrap_err();
        assert!(format!("{:?}", err).contains("parent is not a nominal object"));
    }

    #[test]
    fn sym_module_non_nominal_nominal_arg_errors_cleanly() {
        let m = SymModule::new();
        let root = call(&m, "sym", &[]).expect("sym() mints");
        let err = call(&m, "instanceof", &[root, json!(42)]).unwrap_err();
        assert!(format!("{:?}", err).contains("must be a nominal object"));
    }

    #[test]
    fn sym_module_default_is_a_fresh_sym_module() {
        // `Default` is the documented construction path for registration
        // lists; it must produce a working module, not a panic placeholder.
        let m = SymModule::default();
        assert_eq!(m.name(), SYM_MODULE_NAME);
        assert!(m.function("sym").is_some());
        assert!(m.function("instanceof").is_some());
    }

    #[test]
    fn sym_module_functions_lists_both_aliases() {
        // `functions()` enumerates the module's public surface: `sym` and
        // its `seeded` alias once, `instanceof` under both its surface
        // name and the primitive name.
        let m = SymModule::new();
        let mut names: Vec<&str> = m.functions().into_iter().map(|(n, _)| n).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["instance_of", "instanceof", "seeded", "sym"]);
    }

    #[test]
    fn sym_module_seeded_mints_are_deterministic() {
        // `sym("seed")` / `sym.seeded("seed")` — the ADR-0154 §1
        // deterministic mint: same seed, same handle, every call.
        let m = SymModule::new();
        let a = call(&m, "sym", &[json!("type.Kilo")]).expect("seeded mint");
        let b = call(&m, "seeded", &[json!("type.Kilo")]).expect("seeded mint");
        let c = call(&m, "sym", &[json!("type.Other")]).expect("seeded mint");
        assert_eq!(a, b, "same seed → same nominal (content-hash handle)");
        assert_ne!(a, c, "distinct seeds → distinct nominals");
        assert_eq!(a["__sym__"]["name"], json!("Kilo"));
        // Two `sym()` mints are still distinct (fresh counter).
        let x = call(&m, "sym", &[]).unwrap();
        let y = call(&m, "sym", &[]).unwrap();
        assert_ne!(x, y);
    }

    #[test]
    fn sym_module_seeded_with_links_mints_parent_and_type() {
        // `sym("seed", parent, ty)` — the declared-type mint carrying
        // both links (ADR-0154 §2).
        let m = SymModule::new();
        let number = call(&m, "sym", &[json!("type.Number")]).unwrap();
        let ty = call(&m, "sym", &[json!("type.Type")]).unwrap();
        let int = call(
            &m,
            "sym",
            &[json!("type.Int"), number.clone(), ty.clone()],
        )
        .expect("seeded mint with links");
        // `parent` and `type` links serialize in the payload.
        assert_eq!(
            int["__sym__"]["parent"]["__sym__"]["handle"],
            number["__sym__"]["handle"]
        );
        assert_eq!(
            int["__sym__"]["type"]["__sym__"]["handle"],
            ty["__sym__"]["handle"]
        );
        // `instanceof` reads both: `Int <: Number` (parent walk) and
        // `Int : Type` (membership via the ty link).
        assert_eq!(
            call(&m, "instanceof", &[int.clone(), number.clone()]),
            Ok(json!(true))
        );
        assert_eq!(
            call(&m, "instanceof", &[int.clone(), ty.clone()]),
            Ok(json!(true))
        );
    }

    #[test]
    fn sym_module_instanceof_literals_and_builtin_graph() {
        // The builtin `type.*` graph + literal defaults (ADR-0154 §5):
        // `1 instanceof Nat/Int/Number`, `1.5 instanceof Float`.
        let m = SymModule::new();
        let nat = call(&m, "sym", &[json!("type.Nat")]).unwrap();
        let int = call(&m, "sym", &[json!("type.Int")]).unwrap();
        let number = call(&m, "sym", &[json!("type.Number")]).unwrap();
        let float = call(&m, "sym", &[json!("type.Float")]).unwrap();
        let strs = call(&m, "sym", &[json!("type.Str")]).unwrap();
        for nominal in [&nat, &int, &number] {
            assert_eq!(
                call(&m, "instanceof", &[json!(1), nominal.clone()]),
                Ok(json!(true))
            );
        }
        assert_eq!(
            call(&m, "instanceof", &[json!(1), float.clone()]),
            Ok(json!(false))
        );
        assert_eq!(
            call(&m, "instanceof", &[json!(-1), nat.clone()]),
            Ok(json!(false))
        );
        assert_eq!(
            call(&m, "instanceof", &[json!(-1), int.clone()]),
            Ok(json!(true))
        );
        assert_eq!(
            call(&m, "instanceof", &[json!(1.5), float.clone()]),
            Ok(json!(true))
        );
        assert_eq!(
            call(&m, "instanceof", &[json!("s"), strs.clone()]),
            Ok(json!(true))
        );
    }

    #[test]
    fn sym_module_instanceof_legacy_tag_bridge_still_walks() {
        // `__tag__` data bridge (kept until tag deletion): a tagged
        // datum reaches the seeded nominal graph AND the legacy
        // presentation-name match.
        let m = SymModule::new();
        let datum = json!({"__tag__": "int", "__registry__": "type", "kind": "primitive"});
        let int = call(&m, "sym", &[json!("type.Int")]).unwrap();
        let number = call(&m, "sym", &[json!("type.Number")]).unwrap();
        let float = call(&m, "sym", &[json!("type.Float")]).unwrap();
        assert_eq!(
            call(&m, "instanceof", &[datum.clone(), int]),
            Ok(json!(true))
        );
        assert_eq!(
            call(&m, "instanceof", &[datum.clone(), number]),
            Ok(json!(true))
        );
        assert_eq!(
            call(&m, "instanceof", &[datum.clone(), float]),
            Ok(json!(false))
        );
        // Hand-minted named nominals still bridge by name.
        let named = json!({"__sym__": {"handle": 301, "name": "Int", "parent": null}});
        assert_eq!(
            call(&m, "instanceof", &[datum.clone(), named.clone()]),
            Ok(json!(true))
        );
        // Unknown tags never bridge.
        let unknown = json!({"__tag__": "custom", "__registry__": "type"});
        assert_eq!(
            call(&m, "instanceof", &[unknown, named]),
            Ok(json!(false))
        );
    }
}
