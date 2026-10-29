use std::sync::Arc;

use handl_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use serde_json::Value;
use serde_yaml::Value as YamlValue;

/// The handl source declaring the yaml `op` interface — the single source
/// of truth for the yaml API surface. Loaded at compile time from the
/// kit-stdlib data dir.

/// The op names declared in `yaml.hnl`.
pub const YAML_OPS: &[&str] = &["yaml.parse", "yaml.stringify", "yaml.yaml"];

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
        handl_value::check_portable(&value)
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
        handl_value::check_portable(&args[0])
            .map_err(|e| EvalError::Other(format!("yaml stringify error: {e}")))?;
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
    registry.register("yaml.yaml", Arc::new(YamlName));
    registry
}
/// to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy)]
pub struct YamlName;

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
