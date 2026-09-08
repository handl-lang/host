//! # handl-lib-toml
//!
//! TOML `Encoder`/`Decoder` for the `handl-lib-serialize` framework, and
//! `toml.parse`/`toml.stringify` ops for the handl runtime.
//!
//! Backed by `toml`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The toml interface is declared in
//! `handl-packages/toml/toml.hnl` via `op` declarations, and the Rust
//! parse/stringify logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`TomlLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Module`/`Plugin` trait scaffolding.
//!
//! The [`Toml`] codec and [`TomlModule`] re-export are preserved for backward
//! compatibility with [`plugin-lib-utils`], which uses [`Toml`] in its
//! `SerializerRegistry` and registers [`TomlModule`] directly.

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use handl_plugin_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use serde_json::Value;
use toml::Value as TomlValue;

/// The handl source declaring the toml `op` interface — the single source
/// of truth for the toml API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const TOML_HNL: &str = include_str!("../../../handl-packages/toml/toml.hnl");

/// The op names declared in `toml.hnl`.
const TOML_OPS: &[&str] = &["toml.parse", "toml.stringify"];

// ---------------------------------------------------------------------------
// Toml codec (backward compat for plugin-lib-utils SerializerRegistry)
// ---------------------------------------------------------------------------

/// Combined TOML codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Toml;

/// TOML encoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct TomlEncoder;

/// TOML decoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct TomlDecoder;

impl Encoder<TomlValue> for Toml {
    type Output = String;

    fn encode(&self, value: &TomlValue) -> Result<Self::Output, SerializeError> {
        toml::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Decoder<str, TomlValue> for Toml {
    fn decode(&self, input: &str) -> Result<TomlValue, SerializeError> {
        toml::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Serializer for Toml {
    fn serialize(&self, value: &Value) -> Result<String, SerializeError> {
        toml::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }

    fn deserialize(&self, input: &str) -> Result<Value, SerializeError> {
        toml::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Encoder<TomlValue> for TomlEncoder {
    type Output = String;

    fn encode(&self, value: &TomlValue) -> Result<Self::Output, SerializeError> {
        Toml.encode(value)
    }
}

impl Decoder<str, TomlValue> for TomlDecoder {
    fn decode(&self, input: &str) -> Result<TomlValue, SerializeError> {
        Toml.decode(input)
    }
}

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// The Rust handl for `op "toml.parse" : String -> Value`.
///
/// Parses a TOML string into a value. This is the eval-time `Function` impl
/// registered under the op name `"toml.parse"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct TomlParse;

impl Function for TomlParse {
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
            .ok_or_else(|| EvalError::Other("toml.parse expects a string".to_string()))?;
        let value: Value =
            toml::from_str(s).map_err(|e| EvalError::Other(format!("toml parse error: {e}")))?;
        Ok(value)
    }
}

/// The Rust handl for `op "toml.stringify" : Value -> String`.
///
/// Serializes a value to a TOML string. This is the eval-time `Function`
/// impl registered under the op name `"toml.stringify"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct TomlStringify;

impl Function for TomlStringify {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1,
                args.len()
            )));
        }
        let s = toml::to_string(&args[0])
            .map_err(|e| EvalError::Other(format!("toml stringify error: {e}")))?;
        Ok(Value::String(s))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for toml, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"toml.parse"` → [`TomlParse`] (no runtime checker; axioms trusted)
/// - `"toml.stringify"` → [`TomlStringify`] (no runtime checker; axioms trusted)
pub fn toml_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("toml.parse", Arc::new(TomlParse));
    registry.register("toml.stringify", Arc::new(TomlStringify));
    registry
}

// ---------------------------------------------------------------------------
// TomlModule (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

/// The `toml` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// (e.g. [`plugin-lib-utils`]) that register `TomlModule` directly. The
/// module exposes `parse` and `stringify` functions that dispatch to the
/// [`OpRegistry`], plus the legacy `toml` function that returns the
/// serializer name (for `serialize.serialize`/`serialize.deserialize`).
#[derive(Debug, Clone, Copy, Default)]
pub struct TomlModule;

impl Module for TomlModule {
    fn name(&self) -> &str {
        "toml"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "parse" => Some(&TomlParse),
            "stringify" => Some(&TomlStringify),
            "toml" => Some(&TomlName),
            _ => None,
        }
    }
}

/// Legacy `toml` function — returns the serializer name so it can be passed
/// to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy)]
struct TomlName;

impl Function for TomlName {
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
            "type": "serializer",
            "name": "toml",
        })))
    }
}

// ---------------------------------------------------------------------------
// TomlLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `toml` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The toml interface is declared in `toml.hnl` via `op`/`handl`
/// declarations; the Rust parse/stringify handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct TomlLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl TomlLibraryPlugin {
    /// Create a new `TomlLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(toml_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps("toml", "lib.toml", "lib-toml", TOML_HNL, &[])
                .with_ops(TOML_OPS, registry),
        }
    }
}

impl Default for TomlLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for TomlLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for toml with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`TomlLibraryPlugin`].
pub fn toml_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(toml_op_registry());
    HandlLibraryPlugin::new_with_caps("toml", "lib.toml", "lib-toml", TOML_HNL, &[])
        .with_ops(TOML_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`TomlLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_toml::LibraryPlugin`.
pub use TomlLibraryPlugin as LibraryPlugin;

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
    fn round_trip_document() {
        let input = r#"
title = "example"
[owner]
name = "luis"
nums = [1, 2, 3]
active = true
"#;
        let value = Toml.decode(input).unwrap();
        let output = Toml.encode(&value).unwrap();
        let round = Toml.decode(&output).unwrap();
        assert_eq!(value, round);
    }

    #[test]
    fn rejects_invalid_toml() {
        assert!(Toml.decode("not toml").is_err());
    }

    #[test]
    fn parse_works() {
        let func = TomlParse;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!("title = \"example\"")], &mut ctx)
            .unwrap_or_else(|e| panic!("parse failed: {e}"));
        assert_eq!(result, json!({"title": "example"}));
    }

    #[test]
    fn stringify_works() {
        let func = TomlStringify;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!({"title": "example"})], &mut ctx)
            .unwrap_or_else(|e| panic!("stringify failed: {e}"));
        assert!(result.as_str().unwrap().contains("title = \"example\""));
    }

    #[test]
    fn parse_stringify_roundtrip() {
        let parse = TomlParse;
        let stringify = TomlStringify;
        let mut ctx = EvalContext::new();
        let original = json!("title = \"example\"\nnums = [1, 2, 3]\n");
        let value = parse.call(&[original.clone()], &mut ctx).unwrap();
        let text = stringify.call(&[value.clone()], &mut ctx).unwrap();
        let round = parse.call(&[text], &mut ctx).unwrap();
        assert_eq!(round, value);
    }

    #[test]
    fn parse_rejects_non_string() {
        let func = TomlParse;
        let mut ctx = EvalContext::new();
        let err = func.call(&[json!(42)], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("expects a string"));
    }

    #[test]
    fn parse_rejects_invalid_toml() {
        let func = TomlParse;
        let mut ctx = EvalContext::new();
        let err = func.call(&[json!("not toml = ")], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("toml parse error"));
    }

    #[test]
    fn stringify_rejects_wrong_arity() {
        let func = TomlStringify;
        let mut ctx = EvalContext::new();
        let err = func.call(&[], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("expected 1 args"));
    }

    #[test]
    fn op_registry_registers_both_ops() {
        let registry = toml_op_registry();
        assert!(registry.contains("toml.parse"));
        assert!(registry.contains("toml.stringify"));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn toml_module_exposes_functions() {
        let module = TomlModule;
        assert_eq!(module.name(), "toml");

        let mut ctx = EvalContext::new();
        let parse = module.function("parse").expect("parse should exist");
        let result = parse.call(&[json!("title = \"x\"")], &mut ctx).unwrap();
        assert_eq!(result, json!({"title": "x"}));

        let stringify = module
            .function("stringify")
            .expect("stringify should exist");
        let result = stringify.call(&[json!({"a": 1})], &mut ctx).unwrap();
        assert!(result.as_str().unwrap().contains("a = 1"));
    }

    #[test]
    fn toml_module_legacy_name_function() {
        let module = TomlModule;
        let mut ctx = EvalContext::new();
        let name_func = module.function("toml").expect("toml should exist");
        let result = name_func.call(&[], &mut ctx).unwrap();
        assert_eq!(result, json!({"type": "serializer", "name": "toml"}));
    }

    #[test]
    fn plugin_id_and_name() {
        let plugin = TomlLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.toml");
        assert_eq!(plugin.name(), "lib-toml");
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = toml_handl_plugin();
        assert_eq!(plugin.id(), "lib.toml");
        assert_eq!(plugin.name(), "lib-toml");
    }

    #[test]
    fn toml_module_works_in_module_registry() {
        let module = TomlModule;
        let registry = ModuleRegistry::new().with(module);
        let lookup: &dyn ModuleLookup = &registry;

        let func = lookup
            .lookup("toml.parse")
            .expect("toml.parse should resolve");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!("title = \"hello\"")], &mut ctx).unwrap();
        assert_eq!(result, json!({"title": "hello"}));
    }

    // -----------------------------------------------------------------------
    // ADR-0077 Phase 5: capability declaration tests
    // -----------------------------------------------------------------------

    #[test]
    fn toml_plugin_exposes_declared_capabilities() {
        let plugin = TomlLibraryPlugin::new();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
    }
}
