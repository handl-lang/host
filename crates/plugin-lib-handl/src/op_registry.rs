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
// OpRegistry
// ---------------------------------------------------------------------------

/// A thread-safe registry mapping op names to Rust `Function` impls (the
/// handls).
///
/// This is the host-side counterpart to the `op`/`handl` declaration form
/// (ADR-0077 Phase 2). A `.hnl` file declares `op "base64.encode" : String ->
/// String` and `handl "base64.encode" = rust_base64_encode`; the host crate
/// registers a Rust `Function` impl under the op name `"base64.encode"` via
/// this registry. At eval time, [`OpFunction`] looks up the op name in the
/// registry and dispatches to the registered `Function`.
///
/// The registry is `Send + Sync` and uses interior mutability only at
/// construction time (via `&mut self` on [`OpRegistry::register`]). Once
/// built, it is shared immutably via `Arc<OpRegistry>`.
#[derive(Default)]
pub struct OpRegistry {
    /// Op name → Rust `Function` impl (the handl).
    ops: HashMap<String, Arc<dyn Function + Send + Sync>>,
}

impl OpRegistry {
    /// Create a new, empty `OpRegistry`.
    pub fn new() -> Self {
        Self {
            ops: HashMap::new(),
        }
    }

    /// Register a Rust `Function` impl (the handl) under an op name.
    ///
    /// If an op with the same name was already registered, it is replaced.
    pub fn register(&mut self, op_name: &str, func: Arc<dyn Function + Send + Sync>) {
        self.ops.insert(op_name.to_string(), func);
    }

    /// Look up the `Function` impl registered under an op name.
    ///
    /// Returns `None` if no handl has been registered for this op.
    pub fn get(&self, op_name: &str) -> Option<Arc<dyn Function + Send + Sync>> {
        self.ops.get(op_name).cloned()
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
        func.call(args, ctx)
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
            let n = args[0].as_f64().ok_or_else(|| {
                EvalError::TypeMismatch("expected number".to_string())
            })?;
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
}
