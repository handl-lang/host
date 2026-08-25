//! # handl-lib-base64
//!
//! Base64 encoding for the handl runtime.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_lib_encoding::{Encoding, EncodingError};
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
