//! # handl-lib-base64
//!
//! Base64 encoding for the handl runtime.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 2). The base64 interface is declared in
//! `kit-stdlib/data/base64/base64.hnl` via `op` declarations, and the Rust
//! encode/decode logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`Base64LibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Module`/`Plugin` trait scaffolding.
//!
//! The [`Encoding`] trait impl and [`Base64Module`] re-export are preserved
//! for backward compatibility with [`plugin-lib-utils`], which uses
//! [`Base64`] in its `EncodingRegistry`.

use std::sync::Arc;

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_encoding::{Encoding, EncodingError};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use serde_json::Value;

/// The handl source declaring the base64 `op` interface — the single source
/// of truth for the base64 API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const BASE64_HNL: &str = include_str!("../../../kit-stdlib/data/base64/base64.hnl");

/// The op names declared in `base64.hnl`.
const BASE64_OPS: &[&str] = &["base64.encode", "base64.decode"];

// ---------------------------------------------------------------------------
// Encoding trait impl (backward compat for plugin-lib-utils)
// ---------------------------------------------------------------------------

/// Base64 encoding (implements the [`Encoding`] trait for
/// [`plugin-lib-utils`]'s `EncodingRegistry`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Base64;

impl Encoding for Base64 {
    fn encode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value
            .as_str()
            .ok_or_else(|| EncodingError::Message("base64.encode expects a string".to_string()))?;
        let encoded = BASE64_STANDARD.encode(s.as_bytes());
        Ok(Value::String(encoded))
    }

    fn decode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value
            .as_str()
            .ok_or_else(|| EncodingError::Message("base64.decode expects a string".to_string()))?;
        let decoded = BASE64_STANDARD
            .decode(s)
            .map_err(|e| EncodingError::Message(format!("base64 decode error: {e}")))?;
        let out = String::from_utf8(decoded).map_err(|e| {
            EncodingError::Message(format!("base64 decode is not valid utf-8: {e}"))
        })?;
        Ok(Value::String(out))
    }
}

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// The Rust handl for `op "base64.encode" : String -> String`.
///
/// Encodes a string to base64. This is the eval-time `Function` impl
/// registered under the op name `"base64.encode"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct Base64Encode;

impl Function for Base64Encode {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1,
                args.len()
            )));
        }
        let s = args[0]
            .as_str()
            .ok_or_else(|| EvalError::Other("base64.encode expects a string".to_string()))?;
        let encoded = BASE64_STANDARD.encode(s.as_bytes());
        Ok(Value::String(encoded))
    }
}

/// The Rust handl for `op "base64.decode" : String -> String`.
///
/// Decodes a base64 string. This is the eval-time `Function` impl
/// registered under the op name `"base64.decode"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct Base64Decode;

impl Function for Base64Decode {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1,
                args.len()
            )));
        }
        let s = args[0]
            .as_str()
            .ok_or_else(|| EvalError::Other("base64.decode expects a string".to_string()))?;
        let decoded = BASE64_STANDARD
            .decode(s)
            .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
        let out = String::from_utf8(decoded)
            .map_err(|e| EvalError::Other(format!("base64 decode is not valid utf-8: {e}")))?;
        Ok(Value::String(out))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for base64, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"base64.encode"` → [`Base64Encode`]
/// - `"base64.decode"` → [`Base64Decode`]
pub fn base64_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("base64.encode", Arc::new(Base64Encode));
    registry.register("base64.decode", Arc::new(Base64Decode));
    registry
}

// ---------------------------------------------------------------------------
// Base64Module (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

/// The `base64` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// (e.g. [`plugin-lib-utils`]) that register `Base64Module` directly. The
/// module exposes `encode` and `decode` functions that dispatch to the
/// [`OpRegistry`], plus the legacy `base64` function that returns the codec
/// name (for `encoding.encode`/`encoding.decode`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Base64Module;

impl Module for Base64Module {
    fn name(&self) -> &str {
        "base64"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "encode" => Some(&Base64Encode),
            "decode" => Some(&Base64Decode),
            "base64" => Some(&Base64Name),
            _ => None,
        }
    }
}

/// Legacy `base64` function — returns the codec name so it can be passed to
/// `encoding.encode`/`encoding.decode`.
#[derive(Debug, Clone, Copy)]
struct Base64Name;

impl Function for Base64Name {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                0,
                args.len(),
            )));
        }
        Ok(serde_json::Value::from(serde_json::json!({
            "type": "encoding",
            "name": "base64",
        })))
    }
}

// ---------------------------------------------------------------------------
// Base64LibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `base64` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 2 replacement for the old hand-rolled `LibraryPlugin`.
/// The base64 interface is declared in `base64.hnl` via `op`/`handl`
/// declarations; the Rust encode/decode handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct Base64LibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl Base64LibraryPlugin {
    /// Create a new `Base64LibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(base64_op_registry());
        Self {
            inner: HandlLibraryPlugin::new(
                "base64",
                "lib.base64",
                "lib-base64",
                BASE64_HNL,
                &[],
            )
            .with_ops(BASE64_OPS, registry),
        }
    }
}

impl Default for Base64LibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for Base64LibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for base64 with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`Base64LibraryPlugin`].
pub fn base64_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(base64_op_registry());
    HandlLibraryPlugin::new("base64", "lib.base64", "lib-base64", BASE64_HNL, &[])
        .with_ops(BASE64_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`Base64LibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_base64::LibraryPlugin`.
pub use Base64LibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, Module, ModuleLookup, Plugin};
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    #[test]
    fn encode_works() {
        let func = Base64Encode;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!("hello")], &mut ctx)
            .unwrap_or_else(|e| panic!("encode failed: {e}"));
        assert_eq!(result, json!("aGVsbG8="));
    }

    #[test]
    fn decode_works() {
        let func = Base64Decode;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!("aGVsbG8=")], &mut ctx)
            .unwrap_or_else(|e| panic!("decode failed: {e}"));
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn encode_decode_roundtrip() {
        let encode = Base64Encode;
        let decode = Base64Decode;
        let mut ctx = EvalContext::new();
        let original = json!("Hello, World!");
        let encoded = encode.call(&[original.clone()], &mut ctx).unwrap();
        let decoded = decode.call(&[encoded], &mut ctx).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn op_registry_registers_both_ops() {
        let registry = base64_op_registry();
        assert!(registry.contains("base64.encode"));
        assert!(registry.contains("base64.decode"));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn base64_module_exposes_functions() {
        let module = Base64Module;
        assert_eq!(module.name(), "base64");

        let mut ctx = EvalContext::new();
        let encode = module.function("encode").expect("encode should exist");
        let result = encode.call(&[json!("test")], &mut ctx).unwrap();
        assert_eq!(result, json!("dGVzdA=="));

        let decode = module.function("decode").expect("decode should exist");
        let result = decode.call(&[json!("dGVzdA==")], &mut ctx).unwrap();
        assert_eq!(result, json!("test"));
    }

    #[test]
    fn base64_module_legacy_name_function() {
        let module = Base64Module;
        let mut ctx = EvalContext::new();
        let name_func = module.function("base64").expect("base64 should exist");
        let result = name_func.call(&[], &mut ctx).unwrap();
        assert_eq!(
            result,
            json!({"type": "encoding", "name": "base64"})
        );
    }

    #[test]
    fn plugin_id_and_name() {
        let plugin = Base64LibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.base64");
        assert_eq!(plugin.name(), "lib-base64");
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = base64_handl_plugin();
        assert_eq!(plugin.id(), "lib.base64");
        assert_eq!(plugin.name(), "lib-base64");
    }

    #[test]
    fn base64_module_works_in_module_registry() {
        let module = Base64Module;
        let registry = ModuleRegistry::new().with(module);
        let lookup: &dyn ModuleLookup = &registry;

        let func = lookup
            .lookup("base64.encode")
            .expect("base64.encode should resolve");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!("hello")], &mut ctx).unwrap();
        assert_eq!(result, json!("aGVsbG8="));
    }
}
