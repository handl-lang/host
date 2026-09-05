//! The `tar` handl runtime module.
//!
//! Exposes tar archive helpers callable from handl expressions:
//!
//! - `tar.pack(array)` — packs an array of `{path, contents}` objects
//!   into a tar archive, returning the archive as a base64 string.
//! - `tar.unpack(string)` — unpacks a base64 tar archive string into
//!   an array of `{path, contents}` objects where `contents` is base64.

use std::sync::{Arc, OnceLock};

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_archive::{Archive, ArchiveEntry};
use handl_plugin_lib_handl::HandlModule;
use serde_json::Value as JsonValue;

use crate::TarArchive;

/// The handl source declaring the tar `op` interface — the single source
/// of truth for the tar API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const ARCHIVE_TAR_HNL: &str = include_str!("../../../handl-packages/archive_tar/archive_tar.hnl");

/// The op names declared in `archive_tar.hnl`.
const ARCHIVE_TAR_OPS: &[&str] = &["tar.pack", "tar.unpack"];

/// Build the [`HandlModule`] for tar.
///
/// Both functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the [`HandlModule`] dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_tar_module() -> HandlModule {
    let registry = Arc::new(crate::archive_tar_op_registry());
    HandlModule::new("tar", ARCHIVE_TAR_HNL, &[]).with_ops(ARCHIVE_TAR_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for tar.
static TAR_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for tar, initializing it on first call.
fn tar_module() -> &'static HandlModule {
    TAR_MODULE.get_or_init(build_tar_module)
}

/// The `tar` module — tar archive pack/unpack for handl expressions.
///
/// The interface is declared in `archive_tar.hnl` via `op`/`handl`
/// declarations and loaded via the generic [`HandlModule`]; this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct TarModule;

impl Module for TarModule {
    fn name(&self) -> &str {
        "tar"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        tar_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        tar_module().functions()
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
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1,
            args.len(),
        )))?;

        let items = arg
            .as_array()
            .map(Vec::as_slice)
            .ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "array", "non-array",
            )))?;

        let mut entries = Vec::with_capacity(items.len());

        for item in items {
            let obj = item.as_object().ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "object", "non-object",
            )))?;

            let path = obj
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or(EvalError::Other("tar.pack entry missing path".to_string()))?;

            let contents = obj
                .get("contents")
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
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1,
            args.len(),
        )))?;

        let s = arg.as_str().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))?;

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
