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
use handl_value::{sym_from_parent_value, tagged_to_value, TaggedValue};
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
            return Ok(callee
                .get("body")
                .cloned()
                .unwrap_or(Value::Null));
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
        let kind = args
            .get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| EvalError::Other("lookup_eval_scheme: expected kind string".to_string()))?;
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

/// `sym()` / `sym(parent)` — mint a fresh nominal identity (ADR-0106 §2).
///
/// - `sym()` — fresh nominal identity with no parent (a root sym).
/// - `sym(A)` — fresh nominal identity whose nominal parent is `A`; `A`
///   must itself be a nominal object (`{"__sym__": ...}`), otherwise a
///   `TypeMismatch` error is returned.
///
/// The returned value is the explicit JSON encoding
/// `{"__sym__": {"handle": <n>, "name": "...", "parent": <sym or null>}}`.
/// Identity is the `handle`, minted from a global counter: two distinct
/// `sym()` calls are never equal, even though both display the same
/// presentation name (`"sym"` — names are presentation only).
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
            1 => sym_from_parent_value(DEFAULT_NAME, &TaggedValue::from(args[0].clone()))
                .map_err(EvalError::TypeMismatch)?,
            n => {
                return Err(EvalError::Other(format!(
                    "sym: expected 0 or 1 argument (sym() or sym(parent)), got {}",
                    n
                )))
            }
        };
        Ok(tagged_to_value(&tv))
    }
}

/// `instance_of(value, nominal)` — instanceof over the nominal parent chain
/// (ADR-0106 §2).
///
/// Walks `value`'s nominal parent chain (inclusive of the value itself):
/// `Float instanceof Number` and `Float instanceof Float` are both true;
/// `Number instanceof Float` is false (parent→child is not an instance).
///
/// - The `nominal` argument must itself be a nominal object; otherwise a
///   `TypeMismatch` error is returned.
/// - A non-nominal `value` is simply not an instance of any nominal:
///   returns `Bool(false)` (no error).
///
/// TODO(ADR-0106 Phase 3/4): built-in tagged values from `type.hnl`
/// (`{"__tag__": "int", "__registry__": "type", ...}` etc.) do not yet
/// carry nominal identities. Once `type.hnl` migrates to `sym()`-parented
/// identities (`Float := sym(Number)`, `Int := sym(Number)`) and the tag
/// registry maps value tags onto those identities, `instance_of` should map
/// a tagged value (e.g. `3.14` with `__tag__: "float"`) onto its nominal
/// identity so `instanceof` works for built-in scalars too. Until then,
/// `instance_of` only recognizes first-class `{"__sym__": ...}` values.
struct InstanceOfFn;

impl Function for InstanceOfFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let value = args.get(0).cloned().unwrap_or(Value::Null);
        let nominal = args.get(1).cloned().ok_or_else(|| {
            EvalError::Other("instance_of: expected two arguments (value, nominal)".to_string())
        })?;
        let nominal_tv = TaggedValue::from(nominal);
        if !nominal_tv.is_sym() {
            return Err(EvalError::TypeMismatch(
                "instance_of: second argument must be a nominal object (see ADR-0106 §2)"
                    .to_string(),
            ));
        }
        let value_tv = TaggedValue::from(value);
        Ok(Value::Bool(handl_value::has_ancestor(
            &value_tv, &nominal_tv,
        )))
    }
}

// ---------------------------------------------------------------------------
// VmPrimitivesModule — the Module that registers all VM primitive host functions
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
            push_scope_fn: PushScopeFn { state: Arc::clone(&state) },
            pop_scope_fn: PopScopeFn { state: Arc::clone(&state) },
            bind_fn: BindFn { state: Arc::clone(&state) },
            lookup_fn: LookupFn { state: Arc::clone(&state) },
            make_closure_fn: MakeClosureFn,
            call_fn: CallFn,
            vm_pattern_match_fn: VmPatternMatchFn,
            lookup_eval_scheme_fn: LookupEvalSchemeFn,
            eval_scheme_arr_fn: EvalSchemeArrFn,
            eval_scheme_const_fn: EvalSchemeConstFn,
            ffi_fn: FfiFn,
            setfield_fn: SetFieldFn { state: Arc::clone(&state) },
            ctx_fn: CtxFn { state: Arc::clone(&state) },
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
            ("vm_pattern_match", &self.vm_pattern_match_fn as &dyn Function),
            ("lookup_eval_scheme", &self.lookup_eval_scheme_fn as &dyn Function),
            ("eval_scheme_Arr", &self.eval_scheme_arr_fn as &dyn Function),
            ("eval_scheme_Const", &self.eval_scheme_const_fn as &dyn Function),
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
            "sym" => self.inner.function("sym"),
            "instanceof" => self.inner.function("instance_of"),
            "instance_of" => self.inner.function("instance_of"),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        let mut out = Vec::new();
        if let Some(f) = self.inner.function("sym") {
            out.push(("sym", f));
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
        assert_eq!(call(&m, "instanceof", &[child.clone(), root.clone()]), Ok(json!(true)));
        assert_eq!(call(&m, "instance_of", &[child.clone(), child.clone()]), Ok(json!(true)));
        assert_eq!(call(&m, "instanceof", &[root.clone(), child.clone()]), Ok(json!(false)));
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
}

