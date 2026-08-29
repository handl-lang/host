//! The `gzip` handl runtime module.
//!
//! Exposes gzip compression helpers callable from handl expressions:
//!
//! - `gzip.compress(string)` — base64-encoded gzipped bytes
//! - `gzip.decompress(base64_string)` — original string, or base64 if not UTF-8

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_plugin_lib_compress::{Compressor, Decompressor};
use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value as JsonValue;

use crate::Gzip;

/// The `gzip` module.
#[derive(Clone, Copy)]
pub struct GzipModule;

impl Module for GzipModule {
    fn name(&self) -> &str {
        "gzip"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "compress" => Some(&Compress),
            "decompress" => Some(&Decompress),
            _ => None,
        }
    }
}

fn require_one_string(args: &[NonFunctionValue]) -> Result<String, Error> {
    let arg = args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: args.len(),
    })?;
    arg.as_str()
        .map(|s| s.to_string())
        .ok_or(Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        })
}

fn string_(value: impl Into<String>) -> NonFunctionValue {
    NonFunctionValue::new(JsonValue::String(value.into()))
}

#[derive(Clone, Copy)]
pub struct Compress;

impl Function<NonFunctionValue> for Compress {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let s = require_one_string(args)?;
        let compressed = Gzip::new()
            .compress(s.as_bytes())
            .map_err(|e| Error::PrimitiveError(format!("gzip compress error: {e}")))?;
        Ok(string_(BASE64_STANDARD.encode(&compressed)))
    }
}

#[derive(Clone, Copy)]
pub struct Decompress;

impl Function<NonFunctionValue> for Decompress {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let s = require_one_string(args)?;
        let decoded = BASE64_STANDARD.decode(s).map_err(|e| {
            Error::PrimitiveError(format!("gzip decompress base64 decode error: {e}"))
        })?;
        let decompressed = Gzip::new()
            .decompress(&decoded)
            .map_err(|e| Error::PrimitiveError(format!("gzip decompress error: {e}")))?;
        match String::from_utf8(decompressed) {
            Ok(utf8) => Ok(string_(utf8)),
            Err(e) => Ok(string_(BASE64_STANDARD.encode(e.as_bytes()))),
        }
    }
}
