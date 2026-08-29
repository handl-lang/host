//! # handl-lib-hex
//!
//! Hexadecimal encoding for the handl runtime.

use handl_plugin_lib_encoding::{Encoding, EncodingError};
use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::NonFunctionValue;
use serde_json::Value;

/// Hexadecimal encoding.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hex;

impl Encoding for Hex {
    fn encode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value
            .as_str()
            .ok_or_else(|| EncodingError::Message("hex.encode expects a string".to_string()))?;
        let encoded = hex::encode(s.as_bytes());
        Ok(Value::String(encoded))
    }

    fn decode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value
            .as_str()
            .ok_or_else(|| EncodingError::Message("hex.decode expects a string".to_string()))?;
        let decoded =
            hex::decode(s).map_err(|e| EncodingError::Message(format!("hex decode error: {e}")))?;
        let out = String::from_utf8(decoded)
            .map_err(|e| EncodingError::Message(format!("hex decode is not valid utf-8: {e}")))?;
        Ok(Value::String(out))
    }
}

/// The `hex` module.
///
/// Exposes `hex` as a zero-arg function that returns the codec name
/// so it can be passed to `encoding.encode`/`encoding.decode`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HexModule;

impl Module for HexModule {
    fn name(&self) -> &str {
        "hex"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "hex" => Some(&HexName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct HexName;

impl Function<NonFunctionValue> for HexName {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_plugin_vm::Error<NonFunctionValue>> {
        if !args.is_empty() {
            return Err(handl_plugin_vm::Error::ArgumentCount {
                expected: 0,
                got: args.len(),
            });
        }
        Ok(NonFunctionValue::new(serde_json::json!({
            "type": "encoding",
            "name": "hex",
        })))
    }
}

pub mod plugin;
pub use plugin::LibraryPlugin;
