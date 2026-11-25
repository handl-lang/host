//! `ffi.rust.resolve` — dynamic host-atom resolution (P-013 T-152, §5).
//!
//! The uniform host-fn boundary for cases where atoms aren't statically
//! known (plugins, wasm hosts, generated clients):
//!
//! ```hnl
//! f := ffi.rust.resolve("math.abs")   -- runtime lookup → {fn, sig} arrow
//! x := f(-3)
//! ```
//!
//! `resolve` takes the qualified atom name (`"mod.fn"`, the same dispatch
//! key `atoms.yaml`/`op` decls use), checks it against the registered
//! module table, and returns the [`wrap_dep_fn_with_sig`] wrapper arrow —
//! the `{fn, sig}` foreign-arrow shape `ffi` shares with
//! `from "mod" import f`. Eval is unchanged: the arrow's `callv` body
//! re-resolves `"mod.fn"` through the *applying* runtime's module chain
//! at `call_host` (ambient re-resolution, ADR-0155), so scoped deps and
//! env-instantiated modules apply exactly as they do for a statically
//! imported binding. `sig` is read off the resolved [`Module`]'s
//! [`TypeSignature`] when the atom declares one (P-013 T-148) and rides
//! the arrow for typecheck/`arrow.sig` introspection.
//!
//! Resolution scope is the **kit module table** ([`KitResolver`] — the
//! same table `build_registry` materializes into the eval-time
//! `ModuleLookup`): name → module → `function(local)` mirrors
//! `ModuleRegistry::lookup`'s split-once fallback. Atoms contributed by
//! non-kit plugins that never entered the table are not visible here —
//! the applying chain still resolves them at call time; `resolve` only
//! gates existence + signature at mint time.

use std::rc::Rc;
use std::sync::Arc;

use handl_kit_core::KitResolver;
use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{ModuleResolver, wrap_dep_fn_with_sig};
use handl_value::{TaggedValue, tagged_to_value};
use serde_json::Value;

/// The `ffi.rust.resolve` op: `String ~> (a ~> b)` — qualified atom name
/// in, callable `{fn, sig}` arrow value out.
///
/// Unknown names (unregistered module or missing function) raise an
/// eval error rather than minting a dangling wrapper, so typos fail at
/// the `resolve` call site instead of at the deferred `call_host`.
pub struct FfiRustResolve;

impl FfiRustResolve {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FfiRustResolve {
    fn default() -> Self {
        Self::new()
    }
}

/// Split `"mod.fn"` into `(module, local)` the way
/// `ModuleRegistry::lookup`'s fallback does — first `.` only, so a
/// namespaced local (`"ffi.rust.resolve"` → module `"ffi"`, function
/// `"rust.resolve"`) round-trips.
fn split_atom_name(name: &str) -> Option<(&str, &str)> {
    name.split_once('.').filter(|(_, f)| !f.is_empty())
}

/// Resolve `name` through the kit module table to its `(module, local)`
/// pair — the shared existence gate behind both `ffi.rust.resolve` and
/// [`AtomHandle::resolve`]. Unknown modules and missing functions raise
/// [`EvalError::UnknownFunction`] rather than producing a dangling
/// reference, so typos fail at resolve time instead of at the deferred
/// `call_host`.
fn resolve_module(name: &str) -> Result<(Arc<dyn Module + Send + Sync>, String), EvalError> {
    let (module_name, local) = split_atom_name(name).ok_or_else(|| {
        EvalError::Other(format!(
            "ffi.rust.resolve: `{name}` is not a qualified atom name (expected \"mod.fn\")"
        ))
    })?;
    let module = KitResolver
        .resolve(module_name)
        .ok_or_else(|| EvalError::UnknownFunction(name.to_string()))?;
    if module.function(local).is_none() {
        return Err(EvalError::UnknownFunction(name.to_string()));
    }
    Ok((module, local.to_string()))
}

/// Read the declared signature type value off the resolved module's
/// function, when the atom declares one (P-013 T-148).
fn resolve_sig(module: &dyn Module, local: &str) -> Option<Value> {
    module
        .function(local)?
        .as_type_signature()
        .map(|ts| ts.type_signature())
}

impl Function for FfiRustResolve {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let name = args.first().and_then(Value::as_str).ok_or_else(|| {
            EvalError::TypeMismatch("ffi.rust.resolve: expected a string atom name".to_string())
        })?;
        let (module, local) = resolve_module(name)?;
        let sig = resolve_sig(module.as_ref(), &local).map(|v| Rc::new(TaggedValue::from(v)));
        Ok(tagged_to_value(&wrap_dep_fn_with_sig(name, sig)))
    }
}

// ---------------------------------------------------------------------------
// AtomHandle — the external/runtime-caller half of `ffi.rust.resolve`
// ---------------------------------------------------------------------------

/// An owned, FFI-callable handle to a resolved atom — the runtime-caller
/// counterpart of the `{fn, sig}` arrow [`FfiRustResolve`] mints.
///
/// Where the in-eval op returns a handl arrow that re-dispatches
/// `"mod.fn"` through the *applying* runtime's module chain at
/// `call_host` (ADR-0155 ambient re-resolution), a handle pins the
/// resolved [`Module`] instance — the same `Arc` [`KitResolver`] hands
/// out — plus the function's local name, and invokes it directly through
/// [`Function::call`]'s `serde_json::Value` codec. This is the shape
/// external FFI hosts consume (PyO3 cdylibs, wasm shims, C-ABI
/// wrappers): one [`AtomHandle::resolve`] per atom name, then
/// [`AtomHandle::call`] per invocation, args/results crossing as JSON
/// values exactly as they do across `ffi.call`/`ffi.bind`.
///
/// Resolution scope and signature gating are identical to
/// `ffi.rust.resolve`: the process-wide kit module table, unknown names
/// rejected up front, `signature()` carrying the T-148 declared type
/// when the atom has one.
///
/// The handle does **not** capture an eval engine or a
/// [`ModuleLookup`]: it is for self-contained host atoms (the
/// `tensor.*`-style registry citizens behind `with_ops`), not for
/// `.hnl` functions that need the module's own runtime to evaluate
/// their bodies. Those keep going through the arrow path.
#[derive(Clone)]
pub struct AtomHandle {
    /// The qualified atom name this handle was resolved from (`"mod.fn"`).
    name: String,
    /// The resolved module instance — kept alive for the handle's life
    /// so the borrowed `&dyn Function` behind [`Module::function`] never
    /// dangles.
    module: Arc<dyn Module + Send + Sync>,
    /// The function's local (in-module) name — what
    /// [`Module::function`] expects.
    local: String,
    /// The atom's declared signature type value (P-013 T-148), when it
    /// declares one — the `sig` half of the `{fn, sig}` shape for
    /// callers that want it without minting the arrow.
    sig: Option<Value>,
}

impl AtomHandle {
    /// Resolve a qualified atom name (`"mod.fn"`) through the kit module
    /// table — the same gate `ffi.rust.resolve` applies at mint time.
    pub fn resolve(name: &str) -> Result<Self, EvalError> {
        let (module, local) = resolve_module(name)?;
        let sig = resolve_sig(module.as_ref(), &local);
        Ok(Self {
            name: name.to_string(),
            module,
            local,
            sig,
        })
    }

    /// The qualified atom name this handle was resolved from.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The declared signature type value, when the atom declares one
    /// (P-013 T-148) — `None` for unsigned atoms.
    pub fn signature(&self) -> Option<&Value> {
        self.sig.as_ref()
    }

    /// Invoke the atom with already-evaluated JSON arguments — the same
    /// `serde_json::Value` codec [`Function::call`] uses inside eval.
    ///
    /// The function is re-resolved through the module on every call
    /// (a `HashMap` lookup), so a module that swaps implementations sees
    /// the swap — matching the arrow path's ambient re-resolution.
    pub fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        self.module
            .function(&self.local)
            .ok_or_else(|| EvalError::UnknownFunction(self.name.clone()))?
            .call(args, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, TypeSignature};
    use serde_json::json;
    use std::sync::Arc;

    /// A native test module exposing `probe.double` (unsigned) —
    /// registered into the shared kit table under a unique name.
    struct ProbeModule;

    struct Double;
    impl Function for Double {
        fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            let n = args[0].as_f64().unwrap_or(0.0);
            Ok(json!(n * 2.0))
        }
    }

    struct Signed;
    impl Function for Signed {
        fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(json!(null))
        }
        fn as_type_signature(&self) -> Option<&dyn TypeSignature> {
            Some(self)
        }
    }
    impl TypeSignature for Signed {
        fn type_signature(&self) -> Value {
            json!({"__tag__": "type.function"})
        }
    }

    impl Module for ProbeModule {
        fn name(&self) -> &str {
            "t152probe"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            match name {
                "double" => Some(&Double),
                "signed" => Some(&Signed),
                _ => None,
            }
        }
    }

    fn ensure_registered() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            handl_kit_core::register_modules(&[("t152probe", |_, _| {
                handl_kit_core::ModuleEntry::Native(Arc::new(ProbeModule))
            })]);
        });
    }

    #[test]
    fn resolves_known_atom_to_arrow_value() {
        ensure_registered();
        let mut ctx = EvalContext::new();
        let out = FfiRustResolve::new()
            .call(&[json!("t152probe.double")], &mut ctx)
            .unwrap();
        // The {fn, sig} wire shape: a `{"__arrow__": true, …}` object the
        // engine decodes back into `TaggedValue::Arrow`.
        assert_eq!(out["__arrow__"], json!(true));
        assert_eq!(out["rest"], json!("args"));
        // Unsigned atom → no sig carried.
        assert_eq!(out["sig"], Value::Null);
        // The callv body re-dispatches the qualified name.
        let body = out["body_def"].to_string();
        assert!(body.contains("t152probe.double"), "body: {body}");
    }

    #[test]
    fn carries_sig_when_atom_declares_one() {
        ensure_registered();
        let mut ctx = EvalContext::new();
        let out = FfiRustResolve::new()
            .call(&[json!("t152probe.signed")], &mut ctx)
            .unwrap();
        assert_eq!(out["sig"]["__tag__"], json!("type.function"));
    }

    #[test]
    fn atom_handle_resolves_calls_and_carries_sig() {
        ensure_registered();
        let mut ctx = EvalContext::new();

        let handle = AtomHandle::resolve("t152probe.double").unwrap();
        assert_eq!(handle.name(), "t152probe.double");
        // Unsigned atom → no signature payload.
        assert!(handle.signature().is_none());
        assert_eq!(handle.call(&[json!(21.0)], &mut ctx).unwrap(), json!(42.0));

        let signed = AtomHandle::resolve("t152probe.signed").unwrap();
        assert_eq!(
            signed.signature().unwrap()["__tag__"],
            json!("type.function")
        );
    }

    #[test]
    fn atom_handle_rejects_unknown_names() {
        ensure_registered();
        assert!(matches!(
            AtomHandle::resolve("no.such"),
            Err(EvalError::UnknownFunction(_))
        ));
        assert!(matches!(
            AtomHandle::resolve("t152probe.missing"),
            Err(EvalError::UnknownFunction(_))
        ));
        assert!(AtomHandle::resolve("bare").is_err());
    }

    #[test]
    fn unknown_module_and_function_are_errors() {
        ensure_registered();
        let mut ctx = EvalContext::new();
        let resolve = FfiRustResolve::new();
        assert!(matches!(
            resolve.call(&[json!("no.such")], &mut ctx),
            Err(EvalError::UnknownFunction(_))
        ));
        assert!(matches!(
            resolve.call(&[json!("t152probe.missing")], &mut ctx),
            Err(EvalError::UnknownFunction(_))
        ));
        // Unqualified name and non-string arg are rejected too.
        assert!(resolve.call(&[json!("bare")], &mut ctx).is_err());
        assert!(resolve.call(&[json!(42)], &mut ctx).is_err());
        assert!(resolve.call(&[], &mut ctx).is_err());
    }
}
