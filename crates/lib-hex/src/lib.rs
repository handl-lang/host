//! # handl-lib-hex
//!
//! Hexadecimal encoding for the handl runtime.

use handl_lib_encoding::{Encoding, EncodingError};
use serde_json::Value;

/// Hexadecimal encoding.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hex;

impl Encoding for Hex {
    fn encode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value.as_str().ok_or_else(|| {
            EncodingError::Message("hex.encode expects a string".to_string())
        })?;
        let encoded = hex::encode(s.as_bytes());
        Ok(Value::String(encoded))
    }

    fn decode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value.as_str().ok_or_else(|| {
            EncodingError::Message("hex.decode expects a string".to_string())
        })?;
        let decoded = hex::decode(s).map_err(|e| {
            EncodingError::Message(format!("hex decode error: {e}"))
        })?;
        let out = String::from_utf8(decoded).map_err(|e| {
            EncodingError::Message(format!("hex decode is not valid utf-8: {e}"))
        })?;
        Ok(Value::String(out))
    }
}
