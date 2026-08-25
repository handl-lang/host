//! # handl-lib-base64
//!
//! Base64 encoding for the handl runtime.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_lib_encoding::{Encoding, EncodingError};
use handl_vm::{Context, Function};
use handl_vm_common::Module;
use handl_vm_values_json::NonFunctionValue;
use serde_json::Value;

/// Base64 encoding.
#[derive(Debug, Clone, Copy, Default)]
pub struct Base64;

impl Encoding for Base64 {
    fn encode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value.as_str().ok_or_else(|| {
            EncodingError::Message("base64.encode expects a string".to_string())
        })?;
        let encoded = BASE64_STANDARD.encode(s.as_bytes());
        Ok(Value::String(encoded))
    }

    fn decode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value.as_str().ok_or_else(|| {
            EncodingError::Message("base64.decode expects a string".to_string())
        })?;
        let decoded = BASE64_STANDARD.decode(s).map_err(|e| {
            EncodingError::Message(format!("base64 decode error: {e}"))
        })?;
        let out = String::from_utf8(decoded).map_err(|e| {
            EncodingError::Message(format!("base64 decode is not valid utf-8: {e}"))
        })?;
        Ok(Value::String(out))
    }
}

/// The `base64` module.
///
/// Exposes `base64` as a zero-arg function that returns the codec name
/// so it can be passed to `encoding.encode`/`encoding.decode`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Base64Module;

impl Module for Base64Module {
    fn name(&self) -> &str {
        "base64"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "base64" => Some(&Base64Name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Base64Name;

impl Function<NonFunctionValue> for Base64Name {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_vm::Error<NonFunctionValue>> {
        if !args.is_empty() {
            return Err(handl_vm::Error::ArgumentCount {
                expected: 0,
                got: args.len(),
            });
        }
        Ok(NonFunctionValue::new("base64"))
    }
}
