//! # handl-lib-yaml
//!
//! YAML `Encoder`/`Decoder` for the `handl-lib-serialize` framework, and
//! `yaml.parse`/`yaml.stringify` ops for the handl runtime.
//!
//! Backed by `serde_yaml`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The yaml interface is declared in
//! `handl-packages/yaml/yaml.hnl` via `op` declarations, and the Rust
//! parse/stringify logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`YamlLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Module`/`Plugin` trait scaffolding.
//!
//! The [`Yaml`] codec and [`YamlModule`] re-export are preserved for backward
//! compatibility with [`plugin-lib-utils`], which uses [`Yaml`] in its
//! `SerializerRegistry` and registers [`YamlModule`] directly.

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use handl_plugin_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use serde_json::Value;
use serde_yaml::Value as YamlValue;

/// The handl source declaring the yaml `op` interface — the single source
/// of truth for the yaml API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const YAML_HNL: &str = include_str!("../../../handl-packages/yaml/yaml.hnl");

/// The op names declared in `yaml.hnl`.
const YAML_OPS: &[&str] = &["yaml.parse", "yaml.stringify"];

// ---------------------------------------------------------------------------
// Yaml codec (backward compat for plugin-lib-utils SerializerRegistry)
// ---------------------------------------------------------------------------

/// Combined YAML codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Yaml;

/// YAML encoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct YamlEncoder;

/// YAML decoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct YamlDecoder;

impl Encoder<YamlValue> for Yaml {
    type Output = String;

    fn encode(&self, value: &YamlValue) -> Result<Self::Output, SerializeError> {
        serde_yaml::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Decoder<str, YamlValue> for Yaml {
    fn decode(&self, input: &str) -> Result<YamlValue, SerializeError> {
        serde_yaml::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Serializer for Yaml {
    fn serialize(&self, value: &Value) -> Result<String, SerializeError> {
        serde_yaml::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }

    fn deserialize(&self, input: &str) -> Result<Value, SerializeError> {
        serde_yaml::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Encoder<YamlValue> for YamlEncoder {
    type Output = String;

    fn encode(&self, value: &YamlValue) -> Result<Self::Output, SerializeError> {
        Yaml.encode(value)
    }
}

impl Decoder<str, YamlValue> for YamlDecoder {
    fn decode(&self, input: &str) -> Result<YamlValue, SerializeError> {
        Yaml.decode(input)
    }
}

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// The Rust handl for `op "yaml.parse" : String -> Value`.
///
/// Parses a YAML string into a value. This is the eval-time `Function` impl
/// registered under the op name `"yaml.parse"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct YamlParse;

impl Function for YamlParse {
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
            .ok_or_else(|| EvalError::Other("yaml.parse expects a string".to_string()))?;
        let value: Value = serde_yaml::from_str(s)
            .map_err(|e| EvalError::Other(format!("yaml parse error: {e}")))?;
        Ok(value)
    }
}

/// The Rust handl for `op "yaml.stringify" : Value -> String`.
///
/// Serializes a value to a YAML string. This is the eval-time `Function`
/// impl registered under the op name `"yaml.stringify"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct YamlStringify;

impl Function for YamlStringify {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1,
                args.len()
            )));
        }
        let s = serde_yaml::to_string(&args[0])
            .map_err(|e| EvalError::Other(format!("yaml stringify error: {e}")))?;
        Ok(Value::String(s))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for yaml, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"yaml.parse"` → [`YamlParse`] (no runtime checker; axioms trusted)
/// - `"yaml.stringify"` → [`YamlStringify`] (no runtime checker; axioms trusted)
pub fn yaml_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("yaml.parse", Arc::new(YamlParse));
    registry.register("yaml.stringify", Arc::new(YamlStringify));
    registry
}

// ---------------------------------------------------------------------------
// YamlModule (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

/// The `yaml` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// (e.g. [`plugin-lib-utils`]) that register `YamlModule` directly. The
/// module exposes `parse` and `stringify` functions that dispatch to the
/// [`OpRegistry`], plus the legacy `yaml` function that returns the
/// serializer name (for `serialize.serialize`/`serialize.deserialize`).
#[derive(Debug, Clone, Copy, Default)]
pub struct YamlModule;

impl Module for YamlModule {
    fn name(&self) -> &str {
        "yaml"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "parse" => Some(&YamlParse),
            "stringify" => Some(&YamlStringify),
            "yaml" => Some(&YamlName),
            _ => None,
        }
    }
}

/// Legacy `yaml` function — returns the serializer name so it can be passed
/// to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy)]
struct YamlName;

impl Function for YamlName {
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
            "name": "yaml",
        })))
    }
}

// ---------------------------------------------------------------------------
// YamlLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `yaml` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The yaml interface is declared in `yaml.hnl` via `op`/`handl`
/// declarations; the Rust parse/stringify handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct YamlLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl YamlLibraryPlugin {
    /// Create a new `YamlLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(yaml_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps("yaml", "lib.yaml", "lib-yaml", YAML_HNL, &[])
                .with_ops(YAML_OPS, registry),
        }
    }
}

impl Default for YamlLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for YamlLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for yaml with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`YamlLibraryPlugin`].
pub fn yaml_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(yaml_op_registry());
    HandlLibraryPlugin::new_with_caps("yaml", "lib.yaml", "lib-yaml", YAML_HNL, &[])
        .with_ops(YAML_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`YamlLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_yaml::LibraryPlugin`.
pub use YamlLibraryPlugin as LibraryPlugin;

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
hello: world
list:
  - 1
  - 2
  - 3
nested:
  answer: 42
"#;
        let value = Yaml.decode(input).unwrap();
        let output = Yaml.encode(&value).unwrap();
        let round = Yaml.decode(&output).unwrap();
        assert_eq!(value, round);
    }

    #[test]
    fn rejects_invalid_yaml() {
        assert!(Yaml.decode("  [").is_err());
    }

    #[test]
    fn parse_works() {
        let func = YamlParse;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!("hello: world\n")], &mut ctx)
            .unwrap_or_else(|e| panic!("parse failed: {e}"));
        assert_eq!(result, json!({"hello": "world"}));
    }

    #[test]
    fn stringify_works() {
        let func = YamlStringify;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!({"hello": "world"})], &mut ctx)
            .unwrap_or_else(|e| panic!("stringify failed: {e}"));
        assert!(result.as_str().unwrap().contains("hello: world"));
    }

    #[test]
    fn parse_stringify_roundtrip() {
        let parse = YamlParse;
        let stringify = YamlStringify;
        let mut ctx = EvalContext::new();
        let original = json!("hello: world\nlist:\n  - 1\n  - 2\n");
        let value = parse.call(&[original.clone()], &mut ctx).unwrap();
        let text = stringify.call(&[value.clone()], &mut ctx).unwrap();
        let round = parse.call(&[text], &mut ctx).unwrap();
        assert_eq!(round, value);
    }

    #[test]
    fn parse_rejects_non_string() {
        let func = YamlParse;
        let mut ctx = EvalContext::new();
        let err = func.call(&[json!(42)], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("expects a string"));
    }

    #[test]
    fn parse_rejects_invalid_yaml() {
        let func = YamlParse;
        let mut ctx = EvalContext::new();
        let err = func.call(&[json!("  [")], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("yaml parse error"));
    }

    #[test]
    fn stringify_rejects_wrong_arity() {
        let func = YamlStringify;
        let mut ctx = EvalContext::new();
        let err = func.call(&[], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("expected 1 args"));
    }

    #[test]
    fn op_registry_registers_both_ops() {
        let registry = yaml_op_registry();
        assert!(registry.contains("yaml.parse"));
        assert!(registry.contains("yaml.stringify"));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn yaml_module_exposes_functions() {
        let module = YamlModule;
        assert_eq!(module.name(), "yaml");

        let mut ctx = EvalContext::new();
        let parse = module.function("parse").expect("parse should exist");
        let result = parse.call(&[json!("hello: world\n")], &mut ctx).unwrap();
        assert_eq!(result, json!({"hello": "world"}));

        let stringify = module
            .function("stringify")
            .expect("stringify should exist");
        let result = stringify.call(&[json!({"a": 1})], &mut ctx).unwrap();
        assert!(result.as_str().unwrap().contains("a: 1"));
    }

    #[test]
    fn yaml_module_legacy_name_function() {
        let module = YamlModule;
        let mut ctx = EvalContext::new();
        let name_func = module.function("yaml").expect("yaml should exist");
        let result = name_func.call(&[], &mut ctx).unwrap();
        assert_eq!(result, json!({"type": "serializer", "name": "yaml"}));
    }

    #[test]
    fn plugin_id_and_name() {
        let plugin = YamlLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.yaml");
        assert_eq!(plugin.name(), "lib-yaml");
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = yaml_handl_plugin();
        assert_eq!(plugin.id(), "lib.yaml");
        assert_eq!(plugin.name(), "lib-yaml");
    }

    #[test]
    fn yaml_module_works_in_module_registry() {
        let module = YamlModule;
        let registry = ModuleRegistry::new().with(module);
        let lookup: &dyn ModuleLookup = &registry;

        let func = lookup
            .lookup("yaml.parse")
            .expect("yaml.parse should resolve");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!("hello: world\n")], &mut ctx).unwrap();
        assert_eq!(result, json!({"hello": "world"}));
    }

    // -----------------------------------------------------------------------
    // ADR-0077 Phase 5: capability declaration tests
    // -----------------------------------------------------------------------

    #[test]
    fn yaml_plugin_exposes_declared_capabilities() {
        let plugin = YamlLibraryPlugin::new();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
    }
}
