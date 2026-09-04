//! # handl-lib-json
//!
//! JSON `Encoder`/`Decoder` for the `handl-lib-serialize` framework, and
//! `json.parse`/`json.stringify` ops for the handl runtime.
//!
//! Backed by `serde_json`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The json interface is declared in
//! `handl-packages/json/json.hnl` via `op` declarations, and the Rust
//! parse/stringify logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`JsonLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Module`/`Plugin` trait scaffolding.
//!
//! The [`Json`] codec and [`JsonModule`] re-export are preserved for backward
//! compatibility with [`plugin-lib-utils`], which uses [`Json`] in its
//! `SerializerRegistry` and registers [`JsonModule`] directly.

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use handl_plugin_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use serde_json::Value;

/// The handl source declaring the json `op` interface — the single source
/// of truth for the json API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const JSON_HNL: &str = include_str!("../../../handl-packages/json/json.hnl");

/// The op names declared in `json.hnl`.
const JSON_OPS: &[&str] = &["json.parse", "json.stringify"];

// ---------------------------------------------------------------------------
// Json codec (backward compat for plugin-lib-utils SerializerRegistry)
// ---------------------------------------------------------------------------

/// Combined JSON codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json;

/// JSON encoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonEncoder;

/// JSON decoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonDecoder;

impl Encoder<Value> for Json {
    type Output = String;

    fn encode(&self, value: &Value) -> Result<Self::Output, SerializeError> {
        serde_json::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Decoder<str, Value> for Json {
    fn decode(&self, input: &str) -> Result<Value, SerializeError> {
        serde_json::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Serializer for Json {
    fn serialize(&self, value: &Value) -> Result<String, SerializeError> {
        self.encode(value)
    }

    fn deserialize(&self, input: &str) -> Result<Value, SerializeError> {
        self.decode(input)
    }
}

impl Encoder<Value> for JsonEncoder {
    type Output = String;

    fn encode(&self, value: &Value) -> Result<Self::Output, SerializeError> {
        Json.encode(value)
    }
}

impl Decoder<str, Value> for JsonDecoder {
    fn decode(&self, input: &str) -> Result<Value, SerializeError> {
        Json.decode(input)
    }
}

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// The Rust handl for `op "json.parse" : String -> Value`.
///
/// Parses a JSON string into a value. This is the eval-time `Function` impl
/// registered under the op name `"json.parse"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct JsonParse;

impl Function for JsonParse {
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
            .ok_or_else(|| EvalError::Other("json.parse expects a string".to_string()))?;
        let value: Value = serde_json::from_str(s)
            .map_err(|e| EvalError::Other(format!("json parse error: {e}")))?;
        Ok(value)
    }
}

/// The Rust handl for `op "json.stringify" : Value -> String`.
///
/// Serializes a value to a JSON string. This is the eval-time `Function`
/// impl registered under the op name `"json.stringify"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct JsonStringify;

impl Function for JsonStringify {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1,
                args.len()
            )));
        }
        let s = serde_json::to_string(&args[0])
            .map_err(|e| EvalError::Other(format!("json stringify error: {e}")))?;
        Ok(Value::String(s))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for json, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"json.parse"` → [`JsonParse`] (no runtime checker; axioms trusted)
/// - `"json.stringify"` → [`JsonStringify`] (no runtime checker; axioms trusted)
pub fn json_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("json.parse", Arc::new(JsonParse));
    registry.register("json.stringify", Arc::new(JsonStringify));
    registry
}

// ---------------------------------------------------------------------------
// JsonModule (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

/// The `json` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// (e.g. [`plugin-lib-utils`]) that register `JsonModule` directly. The
/// module exposes `parse` and `stringify` functions that dispatch to the
/// [`OpRegistry`], plus the legacy `json` function that returns the
/// serializer name (for `serialize.serialize`/`serialize.deserialize`).
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonModule;

impl Module for JsonModule {
    fn name(&self) -> &str {
        "json"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "parse" => Some(&JsonParse),
            "stringify" => Some(&JsonStringify),
            "json" => Some(&JsonName),
            _ => None,
        }
    }
}

/// Legacy `json` function — returns the serializer name so it can be passed
/// to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy)]
struct JsonName;

impl Function for JsonName {
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
            "name": "json",
        })))
    }
}

// ---------------------------------------------------------------------------
// JsonLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `json` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The json interface is declared in `json.hnl` via `op`/`handl`
/// declarations; the Rust parse/stringify handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct JsonLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl JsonLibraryPlugin {
    /// Create a new `JsonLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(json_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "json",
                "lib.json",
                "lib-json",
                JSON_HNL,
                &[],
            )
            .with_ops(JSON_OPS, registry),
        }
    }
}

impl Default for JsonLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for JsonLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for json with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`JsonLibraryPlugin`].
pub fn json_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(json_op_registry());
    HandlLibraryPlugin::new_with_caps("json", "lib.json", "lib-json", JSON_HNL, &[])
        .with_ops(JSON_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`JsonLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_json::LibraryPlugin`.
pub use JsonLibraryPlugin as LibraryPlugin;

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
    fn round_trip_object() {
        let input = r#"{"hello":"world","list":[1,2,3]}"#;
        let value = Json.decode(input).unwrap();
        let output = Json.encode(&value).unwrap();
        let round = Json.decode(&output).unwrap();
        assert_eq!(value, round);
    }

    #[test]
    fn round_trip_primitives() {
        for input in ["\"hello\"", "42", "true", "null", "[1,2]"] {
            let value = Json.decode(input).unwrap();
            let output = Json.encode(&value).unwrap();
            let round = Json.decode(&output).unwrap();
            assert_eq!(value, round);
        }
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(Json.decode("not json").is_err());
    }

    #[test]
    fn parse_works() {
        let func = JsonParse;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!("{\"hello\":\"world\"}")], &mut ctx)
            .unwrap_or_else(|e| panic!("parse failed: {e}"));
        assert_eq!(result, json!({"hello": "world"}));
    }

    #[test]
    fn stringify_works() {
        let func = JsonStringify;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!({"hello": "world"})], &mut ctx)
            .unwrap_or_else(|e| panic!("stringify failed: {e}"));
        assert_eq!(result, json!("{\"hello\":\"world\"}"));
    }

    #[test]
    fn parse_stringify_roundtrip() {
        let parse = JsonParse;
        let stringify = JsonStringify;
        let mut ctx = EvalContext::new();
        let original = json!("{\"hello\":\"world\",\"list\":[1,2,3]}");
        let value = parse.call(&[original.clone()], &mut ctx).unwrap();
        let text = stringify.call(&[value.clone()], &mut ctx).unwrap();
        let round = parse.call(&[text], &mut ctx).unwrap();
        assert_eq!(round, value);
    }

    #[test]
    fn parse_rejects_non_string() {
        let func = JsonParse;
        let mut ctx = EvalContext::new();
        let err = func.call(&[json!(42)], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("expects a string"));
    }

    #[test]
    fn parse_rejects_invalid_json() {
        let func = JsonParse;
        let mut ctx = EvalContext::new();
        let err = func.call(&[json!("not json")], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("json parse error"));
    }

    #[test]
    fn stringify_rejects_wrong_arity() {
        let func = JsonStringify;
        let mut ctx = EvalContext::new();
        let err = func.call(&[], &mut ctx).unwrap_err();
        assert!(err.to_string().contains("expected 1 args"));
    }

    #[test]
    fn op_registry_registers_both_ops() {
        let registry = json_op_registry();
        assert!(registry.contains("json.parse"));
        assert!(registry.contains("json.stringify"));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn json_module_exposes_functions() {
        let module = JsonModule;
        assert_eq!(module.name(), "json");

        let mut ctx = EvalContext::new();
        let parse = module.function("parse").expect("parse should exist");
        let result = parse.call(&[json!("42")], &mut ctx).unwrap();
        assert_eq!(result, json!(42));

        let stringify = module
            .function("stringify")
            .expect("stringify should exist");
        let result = stringify.call(&[json!({"a": 1})], &mut ctx).unwrap();
        assert_eq!(result, json!("{\"a\":1}"));
    }

    #[test]
    fn json_module_legacy_name_function() {
        let module = JsonModule;
        let mut ctx = EvalContext::new();
        let name_func = module.function("json").expect("json should exist");
        let result = name_func.call(&[], &mut ctx).unwrap();
        assert_eq!(result, json!({"type": "serializer", "name": "json"}));
    }

    #[test]
    fn plugin_id_and_name() {
        let plugin = JsonLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.json");
        assert_eq!(plugin.name(), "lib-json");
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = json_handl_plugin();
        assert_eq!(plugin.id(), "lib.json");
        assert_eq!(plugin.name(), "lib-json");
    }

    #[test]
    fn json_module_works_in_module_registry() {
        let module = JsonModule;
        let registry = ModuleRegistry::new().with(module);
        let lookup: &dyn ModuleLookup = &registry;

        let func = lookup
            .lookup("json.parse")
            .expect("json.parse should resolve");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!("\"hello\"")], &mut ctx).unwrap();
        assert_eq!(result, json!("hello"));
    }

    // -----------------------------------------------------------------------
    // ADR-0077 Phase 5: capability declaration tests
    // -----------------------------------------------------------------------

    #[test]
    fn json_plugin_exposes_declared_capabilities() {
        let plugin = JsonLibraryPlugin::new();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
    }
}
