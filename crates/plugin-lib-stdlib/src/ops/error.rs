//! `error` ops — the error-raising primitive (ADR-0092 gap (a)).
//!
//! `error.raise(v)` aborts the current evaluation with `v` as the error
//! payload: it never returns a value. The [`OpRegistry`] maps the op name
//! declared in `handl-packages/error/error.hnl` to the Rust handl below.
//!
//! This is the raise half of the ADR-0092 error story. There is still no
//! error-*capture* primitive (`assert.throws`/`test.run` need a
//! handl-reachable `EvalContext::try_call_value`), so a raised error
//! always propagates out of `eval` as a normal eval failure.
//!
//! Payload convention: a `String` payload is the message verbatim; any
//! other value is embedded as its JSON serialization. Errors surface as
//! [`EvalError::Other`], matching how `collections`' `collection is
//! empty` and other contract violations report today.

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use serde_json::Value;

/// The op names declared in `error.hnl`.
pub const ERROR_OPS: &[&str] = &["error.raise"];

/// The Rust handl for `op "error.raise" : Value -> a`.
///
/// Fails loudly: always returns `Err`, never `Ok`. A missing payload
/// defaults to a generic message so `error.raise()` still aborts.
#[derive(Debug, Clone, Copy)]
pub struct Raise;

impl Function for Raise {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let message = match args.first() {
            None => "error raised".to_string(),
            Some(Value::String(s)) => s.clone(),
            Some(v) => serde_json::to_string(v).unwrap_or_else(|_| v.to_string()),
        };
        Err(EvalError::Other(message))
    }
}

/// Build the [`OpRegistry`] for the `error` module.
pub fn error_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("error.raise", Arc::new(Raise));
    registry
}
