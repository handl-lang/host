//! # handl-lib-hex
//!
//! Hexadecimal encoding for the handl runtime.

use handl_plugin_lib_encoding::{Encoding, EncodingError};
use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;
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

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "hex" => Some(&HexName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct HexName;

impl Function for HexName {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 0, args.len(),)));
        }
        Ok(serde_json::Value::from(serde_json::json!({
            "type": "encoding",
            "name": "hex",
        })))
    }
}

pub mod plugin;
pub use plugin::LibraryPlugin;
