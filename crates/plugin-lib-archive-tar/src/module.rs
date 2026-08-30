//! The `tar` handl runtime module.
//!
//! Exposes tar archive helpers callable from handl expressions:
//!
//! - `tar.pack(array)` — packs an array of `{path, contents}` objects
//!   into a tar archive, returning the archive as a base64 string.
//! - `tar.unpack(string)` — unpacks a base64 tar archive string into
//!   an array of `{path, contents}` objects where `contents` is base64.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_plugin_lib_archive::{Archive, ArchiveEntry};
use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;
use serde_json::Value as JsonValue;

use crate::TarArchive;

/// The `tar` module — tar archive pack/unpack for handl expressions.
#[derive(Clone, Copy)]
pub struct TarModule;

impl Module for TarModule {
    fn name(&self) -> &str {
        "tar"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "pack" => Some(&TarPack),
            "unpack" => Some(&TarUnpack),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct TarPack;

impl Function for TarPack {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, args.len(),)))?;

        let items = arg.as_array().map(Vec::as_slice).ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "array", "non-array",)))?;

        let mut entries = Vec::with_capacity(items.len());

        for item in items {
            let obj = item.as_object().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "object", "non-object",)))?;

            let path = obj
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or(EvalError::Other(
                    "tar.pack entry missing path".to_string(),
                ))?;

            let contents =
                obj.get("contents")
                    .and_then(|v| v.as_str())
                    .ok_or(EvalError::Other(
                        "tar.pack entry missing contents".to_string(),
                    ))?;

            let bytes = BASE64_STANDARD
                .decode(contents)
                .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;

            entries.push(ArchiveEntry::new(path, bytes));
        }

        let archive = TarArchive
            .pack(&entries)
            .map_err(|e| EvalError::Other(format!("tar pack error: {e}")))?;

        let encoded = BASE64_STANDARD.encode(&archive);
        Ok(serde_json::Value::from(encoded))
    }
}

#[derive(Clone, Copy)]
pub struct TarUnpack;

impl Function for TarUnpack {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, args.len(),)))?;

        let s = arg.as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))?;

        let bytes = BASE64_STANDARD
            .decode(s)
            .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;

        let entries = TarArchive
            .unpack(&bytes)
            .map_err(|e| EvalError::Other(format!("tar unpack error: {e}")))?;

        let objects: Vec<JsonValue> = entries
            .into_iter()
            .map(|entry| {
                serde_json::json!({
                    "path": entry.path,
                    "contents": BASE64_STANDARD.encode(&entry.contents),
                })
            })
            .collect();

        Ok(serde_json::Value::from(JsonValue::Array(objects)))
    }
}
