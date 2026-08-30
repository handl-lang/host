//! The `gzip` handl runtime module.
//!
//! Exposes gzip compression helpers callable from handl expressions:
//!
//! - `gzip.compress(string)` — base64-encoded gzipped bytes
//! - `gzip.decompress(base64_string)` — original string, or base64 if not UTF-8

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_plugin_lib_compress::{Compressor, Decompressor};
use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;
use serde_json::Value as JsonValue;

use crate::Gzip;

/// The `gzip` module.
#[derive(Clone, Copy)]
pub struct GzipModule;

impl Module for GzipModule {
    fn name(&self) -> &str {
        "gzip"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "compress" => Some(&Compress),
            "decompress" => Some(&Decompress),
            _ => None,
        }
    }
}

fn require_one_string(args: &[serde_json::Value]) -> Result<String, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, args.len(),)))?;
    arg.as_str()
        .map(|s| s.to_string())
        .ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))
}

fn string_(value: impl Into<String>) -> serde_json::Value {
    serde_json::Value::from(JsonValue::String(value.into()))
}

#[derive(Clone, Copy)]
pub struct Compress;

impl Function for Compress {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let compressed = Gzip::new()
            .compress(s.as_bytes())
            .map_err(|e| EvalError::Other(format!("gzip compress error: {e}")))?;
        Ok(string_(BASE64_STANDARD.encode(&compressed)))
    }
}

#[derive(Clone, Copy)]
pub struct Decompress;

impl Function for Decompress {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let decoded = BASE64_STANDARD.decode(s).map_err(|e| {
            EvalError::Other(format!("gzip decompress base64 decode error: {e}"))
        })?;
        let decompressed = Gzip::new()
            .decompress(&decoded)
            .map_err(|e| EvalError::Other(format!("gzip decompress error: {e}")))?;
        match String::from_utf8(decompressed) {
            Ok(utf8) => Ok(string_(utf8)),
            Err(e) => Ok(string_(BASE64_STANDARD.encode(e.as_bytes()))),
        }
    }
}
