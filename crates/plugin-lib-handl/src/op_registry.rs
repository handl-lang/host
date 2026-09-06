//! # Op Registry
//!
//! A thread-safe registry mapping op names (e.g. `"base64.encode"`) to Rust
//! [`Function`](handl_platform::Function) impls — the **handls** (handlers)
//! that implement native operations declared via the `op`/`handl` declaration
//! form (ADR-0077 Phase 2).
//!
//! The registry is `Send + Sync` so it can be shared across threads via
//! [`Arc`](std::sync::Arc). Each op name maps to an `Arc<dyn Function + Send +
//! Sync>`, allowing the same `Function` impl to be cheaply cloned into
//! multiple [`OpFunction`] instances.
//!
//! ## Usage
//!
//! ```ignore
//! use std::sync::Arc;
//! use handl_platform::{EvalContext, EvalError, Function};
//! use handl_plugin_lib_handl::OpRegistry;
//! use serde_json::Value;
//!
//! struct MyEncode;
//! impl Function for MyEncode {
//!     fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
//!         // ... encode logic ...
//!         Ok(Value::String("encoded".to_string()))
//!     }
//! }
//!
//! let mut registry = OpRegistry::new();
//! registry.register("my.encode", Arc::new(MyEncode));
//!
//! let func = registry.get("my.encode").expect("should exist");
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use serde_json::Value;

// ---------------------------------------------------------------------------
// RuntimePredicateChecker
// ---------------------------------------------------------------------------

/// A runtime checker for op axiom predicates (ADR-0077 Phase 3).
///
/// Op authors can optionally provide a checker when registering an op. If
/// provided, the checker is called at eval time by [`OpFunction::call`]:
/// - [`RuntimePredicateChecker::check_requires`] is called **before** the op
///   executes, with the call args. If it returns `Err`, the call is rejected.
/// - [`RuntimePredicateChecker::check_guarantee`] is called **after** the op
///   executes, with the return value. If it returns `Err`, the result is
///   rejected.
///
/// If no checker is registered for an op, its axioms are **trusted** (no
/// runtime check is performed) — the op behaves exactly as before.
///
/// The checker is `Send + Sync` so it can be shared across threads via
/// [`Arc`] alongside the [`OpRegistry`].
pub trait RuntimePredicateChecker: Send + Sync {
    /// Check the `requires` predicate against call arguments.
    ///
    /// Return `Ok(())` if the predicate is satisfied, or `Err(message)` with
    /// a human-readable description of the violation if it is not.
    fn check_requires(&self, args: &[Value]) -> Result<(), String>;

    /// Check the `guarantee` predicate against the return value.
    ///
    /// Return `Ok(())` if the predicate is satisfied, or `Err(message)` with
    /// a human-readable description of the violation if it is not.
    fn check_guarantee(&self, output: &Value) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// OpRegistry
// ---------------------------------------------------------------------------

/// A thread-safe registry mapping op names to Rust `Function` impls (the
/// handls), with optional runtime axiom checkers (ADR-0077 Phase 3).
///
/// This is the host-side counterpart to the `op`/`handl` declaration form
/// (ADR-0077 Phase 2). A `.hnl` file declares `op "base64.encode" : String ->
/// String` and `handl "base64.encode" = rust_base64_encode`; the host crate
/// registers a Rust `Function` impl under the op name `"base64.encode"` via
/// this registry. At eval time, [`OpFunction`] looks up the op name in the
/// registry and dispatches to the registered `Function`.
///
/// In addition (ADR-0077 Phase 3), a [`RuntimePredicateChecker`] may be
/// attached to an op via [`OpRegistry::register_with_checker`]. When present,
/// [`OpFunction::call`] enforces the op's `requires`/`guarantee` axioms at the
/// dispatch boundary: `requires` is checked before the call, `guarantee` is
/// checked against the return value after the call.
///
/// The registry is `Send + Sync` and uses interior mutability only at
/// construction time (via `&mut self` on [`OpRegistry::register`]). Once
/// built, it is shared immutably via `Arc<OpRegistry>`.
#[derive(Default)]
pub struct OpRegistry {
    /// Op name → Rust `Function` impl (the handl).
    ops: HashMap<String, Arc<dyn Function + Send + Sync>>,
    /// Op name → optional runtime axiom checker.
    checkers: HashMap<String, Arc<dyn RuntimePredicateChecker>>,
}

impl OpRegistry {
    /// Create a new, empty `OpRegistry`.
    pub fn new() -> Self {
        Self {
            ops: HashMap::new(),
            checkers: HashMap::new(),
        }
    }

    /// Register a Rust `Function` impl (the handl) under an op name.
    ///
    /// No runtime axiom checker is attached — the op's axioms are trusted
    /// (no runtime check). If an op with the same name was already
    /// registered, it is replaced (and any previously attached checker is
    /// removed).
    pub fn register(&mut self, op_name: &str, func: Arc<dyn Function + Send + Sync>) {
        self.ops.insert(op_name.to_string(), func);
        self.checkers.remove(op_name);
    }

    /// Register a Rust `Function` impl (the handl) under an op name, with an
    /// optional runtime axiom checker (ADR-0077 Phase 3).
    ///
    /// If `checker` is `Some`, [`OpFunction::call`] will enforce the op's
    /// `requires`/`guarantee` axioms at the dispatch boundary. If `None`,
    /// this is equivalent to [`OpRegistry::register`] (axioms trusted).
    ///
    /// If an op with the same name was already registered, it is replaced.
    pub fn register_with_checker(
        &mut self,
        op_name: &str,
        func: Arc<dyn Function + Send + Sync>,
        checker: Option<Arc<dyn RuntimePredicateChecker>>,
    ) {
        self.ops.insert(op_name.to_string(), func);
        if let Some(checker) = checker {
            self.checkers.insert(op_name.to_string(), checker);
        } else {
            self.checkers.remove(op_name);
        }
    }

    /// Look up the `Function` impl registered under an op name.
    ///
    /// Returns `None` if no handl has been registered for this op.
    pub fn get(&self, op_name: &str) -> Option<Arc<dyn Function + Send + Sync>> {
        self.ops.get(op_name).cloned()
    }

    /// Look up the runtime axiom checker for an op, if any (ADR-0077 Phase 3).
    ///
    /// Returns `None` if no checker was attached (the op's axioms are
    /// trusted). Returns `Some` if a checker was registered via
    /// [`OpRegistry::register_with_checker`].
    pub fn get_checker(&self, op_name: &str) -> Option<Arc<dyn RuntimePredicateChecker>> {
        self.checkers.get(op_name).cloned()
    }

    /// Returns `true` if an op with the given name is registered.
    pub fn contains(&self, op_name: &str) -> bool {
        self.ops.contains_key(op_name)
    }

    /// Returns the number of registered ops.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Returns `true` if no ops are registered.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

// ---------------------------------------------------------------------------
// OpFunction
// ---------------------------------------------------------------------------

/// A callable [`Function`] that dispatches to a Rust handl registered in an
/// [`OpRegistry`].
///
/// This is the eval-time bridge between the `op`/`handl` declaration form
/// (ADR-0077 Phase 2) and the existing `ModuleLookup` dispatch. When a
/// `.hnl` file declares `op "base64.encode"` and `handl "base64.encode" =
/// rust_base64_encode`, the loaded [`HandlModule`](crate::HandlModule) creates
/// an `OpFunction` for each op. The `OpFunction` holds the op name and a
/// shared `Arc<OpRegistry>`; on call, it looks up the op name in the registry
/// and dispatches to the registered Rust `Function`.
///
/// This plugs into the existing eval engine without changes: the
/// `OpFunction` is a regular `Function` stored in a `Module`, looked up via
/// `ModuleLookup::lookup(name)`.
pub struct OpFunction {
    /// The op name (e.g. `"base64.encode"`).
    op_name: String,
    /// The shared op registry.
    registry: Arc<OpRegistry>,
}

impl OpFunction {
    /// Create a new `OpFunction` for the given op name and registry.
    pub fn new(op_name: impl Into<String>, registry: Arc<OpRegistry>) -> Self {
        Self {
            op_name: op_name.into(),
            registry,
        }
    }
}

impl Function for OpFunction {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let func = self.registry.get(&self.op_name).ok_or_else(|| {
            EvalError::Other(format!(
                "op '{}': no handl registered (op/handl declaration without a Rust Function impl)",
                self.op_name
            ))
        })?;

        // ADR-0077 Phase 3: enforce `requires`/`guarantee` axioms at the
        // dispatch boundary when a runtime predicate checker is registered.
        let checker = self.registry.get_checker(&self.op_name);

        // `requires` — checked BEFORE the op executes.
        if let Some(ref checker) = checker {
            checker.check_requires(args).map_err(|msg| {
                EvalError::Other(format!(
                    "op {}: requires predicate violated: {msg}",
                    self.op_name
                ))
            })?;
        }

        let output = func.call(args, ctx)?;

        // `guarantee` — checked AFTER the op executes, against the return
        // value.
        if let Some(ref checker) = checker {
            checker.check_guarantee(&output).map_err(|msg| {
                EvalError::Other(format!(
                    "op {}: guarantee predicate violated: {msg}",
                    self.op_name
                ))
            })?;
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Double;
    impl Function for Double {
        fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            let n = args[0]
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number".to_string()))?;
            Ok(json!(n * 2.0))
        }
    }

    #[test]
    fn register_and_get() {
        let mut registry = OpRegistry::new();
        registry.register("test.double", Arc::new(Double));
        assert!(registry.contains("test.double"));
        assert!(!registry.contains("test.missing"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn op_function_dispatches_to_registry() {
        let mut registry = OpRegistry::new();
        registry.register("test.double", Arc::new(Double));
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.double", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(result.as_f64().unwrap(), 42.0);
    }

    #[test]
    fn op_function_missing_handl_returns_error() {
        let registry = OpRegistry::new();
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.missing", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!(1)], &mut ctx);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // ADR-0077 Phase 3: runtime axiom enforcement tests
    // -----------------------------------------------------------------------

    /// A checker that always passes — proves the happy path works.
    struct AlwaysPassChecker;
    impl RuntimePredicateChecker for AlwaysPassChecker {
        fn check_requires(&self, _args: &[Value]) -> Result<(), String> {
            Ok(())
        }
        fn check_guarantee(&self, _output: &Value) -> Result<(), String> {
            Ok(())
        }
    }

    /// A checker that rejects requires when the arg is not a positive number.
    struct RequiresPositiveChecker;
    impl RuntimePredicateChecker for RequiresPositiveChecker {
        fn check_requires(&self, args: &[Value]) -> Result<(), String> {
            let n = args
                .get(0)
                .and_then(Value::as_f64)
                .ok_or("expected a number argument")?;
            if n > 0.0 {
                Ok(())
            } else {
                Err(format!("expected a positive number, got {n}"))
            }
        }
        fn check_guarantee(&self, _output: &Value) -> Result<(), String> {
            Ok(())
        }
    }

    /// A checker that rejects guarantee unless the output is even.
    struct GuaranteeEvenChecker;
    impl RuntimePredicateChecker for GuaranteeEvenChecker {
        fn check_requires(&self, _args: &[Value]) -> Result<(), String> {
            Ok(())
        }
        fn check_guarantee(&self, output: &Value) -> Result<(), String> {
            let n = output.as_f64().ok_or("expected a number output")?;
            if n.fract() == 0.0 && (n as i64) % 2 == 0 {
                Ok(())
            } else {
                Err(format!("expected an even integer, got {n}"))
            }
        }
    }

    #[test]
    fn register_with_checker_attaches_checker() {
        let mut registry = OpRegistry::new();
        registry.register_with_checker(
            "test.double",
            Arc::new(Double),
            Some(Arc::new(AlwaysPassChecker)),
        );
        assert!(registry.get_checker("test.double").is_some());
        // An op registered without a checker has no checker.
        registry.register("test.double", Arc::new(Double));
        assert!(registry.get_checker("test.double").is_none());
    }

    #[test]
    fn op_without_checker_is_trusted() {
        // No checker → behaves exactly as before (no runtime check).
        let mut registry = OpRegistry::new();
        registry.register("test.double", Arc::new(Double));
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.double", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!(-5)], &mut ctx).unwrap();
        assert_eq!(result.as_f64().unwrap(), -10.0);
    }

    #[test]
    fn op_with_passing_checker_succeeds() {
        let mut registry = OpRegistry::new();
        registry.register_with_checker(
            "test.double",
            Arc::new(Double),
            Some(Arc::new(AlwaysPassChecker)),
        );
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.double", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(result.as_f64().unwrap(), 42.0);
    }

    #[test]
    fn op_requires_rejects_violating_call() {
        let mut registry = OpRegistry::new();
        registry.register_with_checker(
            "test.double",
            Arc::new(Double),
            Some(Arc::new(RequiresPositiveChecker)),
        );
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.double", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!(-5)], &mut ctx);
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("requires predicate violated"),
            "expected requires violation in error, got: {msg}"
        );
    }

    #[test]
    fn op_requires_allows_satisfying_call() {
        let mut registry = OpRegistry::new();
        registry.register_with_checker(
            "test.double",
            Arc::new(Double),
            Some(Arc::new(RequiresPositiveChecker)),
        );
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.double", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!(5)], &mut ctx).unwrap();
        assert_eq!(result.as_f64().unwrap(), 10.0);
    }

    #[test]
    fn op_guarantee_rejects_violating_output() {
        // Double of an odd number is even, so use a checker that requires
        // even output and feed an even input that doubles to an even — that
        // passes. To trigger a guarantee violation, use a function that
        // produces an odd output.
        struct OddOutput;
        impl Function for OddOutput {
            fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
                Ok(json!(3))
            }
        }

        let mut registry = OpRegistry::new();
        registry.register_with_checker(
            "test.odd",
            Arc::new(OddOutput),
            Some(Arc::new(GuaranteeEvenChecker)),
        );
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.odd", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!(1)], &mut ctx);
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("guarantee predicate violated"),
            "expected guarantee violation in error, got: {msg}"
        );
    }

    #[test]
    fn op_guarantee_allows_satisfying_output() {
        let mut registry = OpRegistry::new();
        registry.register_with_checker(
            "test.double",
            Arc::new(Double),
            Some(Arc::new(GuaranteeEvenChecker)),
        );
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.double", Arc::clone(&shared));
        let mut ctx = EvalContext::new();
        // 21 * 2 = 42 (even) → guarantee passes.
        let result = op_func.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(result.as_f64().unwrap(), 42.0);
    }

    #[test]
    fn op_requires_and_guarantee_both_enforced() {
        // Requires positive input AND guarantees even output.
        struct BothChecker;
        impl RuntimePredicateChecker for BothChecker {
            fn check_requires(&self, args: &[Value]) -> Result<(), String> {
                let n = args
                    .get(0)
                    .and_then(Value::as_f64)
                    .ok_or("expected number")?;
                if n > 0.0 {
                    Ok(())
                } else {
                    Err(format!("expected positive, got {n}"))
                }
            }
            fn check_guarantee(&self, output: &Value) -> Result<(), String> {
                let n = output.as_f64().ok_or("expected number output")?;
                if n.fract() == 0.0 && (n as i64) % 2 == 0 {
                    Ok(())
                } else {
                    Err(format!("expected even integer, got {n}"))
                }
            }
        }

        let mut registry = OpRegistry::new();
        registry.register_with_checker(
            "test.double",
            Arc::new(Double),
            Some(Arc::new(BothChecker)),
        );
        let shared = Arc::new(registry);

        let op_func = OpFunction::new("test.double", Arc::clone(&shared));
        let mut ctx = EvalContext::new();

        // Valid: positive input → even output.
        let result = op_func.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(result.as_f64().unwrap(), 42.0);

        // Invalid requires: negative input.
        let err = op_func.call(&[json!(-3)], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("requires predicate violated"));
    }
}
