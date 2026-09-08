//! Tar archive ops — consolidated from the deleted `plugin-lib-archive-tar`
//! crate (Phase 5: plugin-lib → handl-packages migration).
//!
//! The `Archive` trait / `ArchiveEntry` / `ArchiveError` scaffolding from
//! `plugin-lib-archive` is dropped entirely — tar pack/unpack is now a direct
//! `Function` impl using the `tar` crate, registered as an op atom in the
//! [`OpRegistry`] exactly like `base64`, `json`, `yaml`, etc.

use std::io::Read;
use std::sync::Arc;

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use serde_json::Value as JsonValue;
use serde_json::json;
use tar::{Archive as TarReader, Builder, Header};

/// The op names declared in `archive_tar.hnl`.
pub const TAR_OPS: &[&str] = &["tar.pack", "tar.unpack"];

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// The Rust handl for `op "tar.pack" : Array -> String`.
///
/// Packs an array of `{path, contents}` objects (where `contents` is base64)
/// into a tar archive, returning the archive as a base64 string.
#[derive(Debug, Clone, Copy)]
pub struct TarPack;

impl Function for TarPack {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let arg = args
            .first()
            .ok_or_else(|| EvalError::TypeMismatch("expected 1 arg, got 0".to_string()))?;

        let items = arg
            .as_array()
            .ok_or_else(|| EvalError::TypeMismatch("tar.pack expects an array".to_string()))?;

        let mut builder = Builder::new(Vec::new());
        for item in items {
            let obj = item.as_object().ok_or_else(|| {
                EvalError::TypeMismatch("tar.pack entry must be an object".to_string())
            })?;

            let path = obj
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| EvalError::Other("tar.pack entry missing path".to_string()))?
                .to_string();

            let contents_val = obj
                .get("contents")
                .ok_or_else(|| EvalError::Other("tar.pack entry missing contents".to_string()))?;

            let contents_str = contents_val.as_str().ok_or_else(|| {
                EvalError::TypeMismatch("tar.pack contents must be a string".to_string())
            })?;

            let bytes = BASE64_STANDARD
                .decode(contents_str)
                .map_err(|e| EvalError::Other(format!("base64 decode: {e}")))?;

            let mut header = Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mtime(0);
            let mut data = std::io::Cursor::new(&bytes);
            builder
                .append_data(&mut header, &path, &mut data)
                .map_err(|e| EvalError::Other(format!("tar append: {e}")))?;
        }

        let archive_bytes = builder
            .into_inner()
            .map_err(|e| EvalError::Other(format!("tar finalize: {e}")))?;

        let encoded = BASE64_STANDARD.encode(&archive_bytes);
        Ok(JsonValue::String(encoded))
    }
}

/// The Rust handl for `op "tar.unpack" : String -> Array`.
///
/// Unpacks a base64 tar archive string into an array of `{path, contents}`
/// objects where `contents` is base64.
#[derive(Debug, Clone, Copy)]
pub struct TarUnpack;

impl Function for TarUnpack {
    fn call(&self, args: &[JsonValue], _ctx: &mut EvalContext) -> Result<JsonValue, EvalError> {
        let arg = args
            .first()
            .ok_or_else(|| EvalError::TypeMismatch("expected 1 arg, got 0".to_string()))?;

        let s = arg
            .as_str()
            .ok_or_else(|| EvalError::TypeMismatch("tar.unpack expects a string".to_string()))?;

        let tar_bytes = BASE64_STANDARD
            .decode(s)
            .map_err(|e| EvalError::Other(format!("base64 decode: {e}")))?;

        let mut archive = TarReader::new(std::io::Cursor::new(tar_bytes));
        let mut objects = Vec::new();

        for entry_result in archive
            .entries()
            .map_err(|e| EvalError::Other(format!("tar entries: {e}")))?
        {
            let mut entry =
                entry_result.map_err(|e| EvalError::Other(format!("tar entry: {e}")))?;
            let path = entry
                .path()
                .map_err(|e| EvalError::Other(format!("tar path: {e}")))?
                .to_string_lossy()
                .into_owned();
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .map_err(|e| EvalError::Other(format!("tar read: {e}")))?;
            objects.push(json!({
                "path": path,
                "contents": BASE64_STANDARD.encode(&contents)
            }));
        }

        Ok(JsonValue::Array(objects))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for tar, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"tar.pack"`   → [`TarPack`]
/// - `"tar.unpack"` → [`TarUnpack`]
pub fn tar_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("tar.pack", Arc::new(TarPack));
    registry.register("tar.unpack", Arc::new(TarUnpack));
    registry
}
