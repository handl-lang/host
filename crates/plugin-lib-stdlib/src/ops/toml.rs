use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use handl_plugin_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use serde_json::Value;
use toml::Value as TomlValue;

/// The handl source declaring the toml `op` interface — the single source
/// of truth for the toml API surface. Loaded at compile time from the
/// kit-stdlib data dir.

/// The op names declared in `toml.hnl`.
pub const TOML_OPS: &[&str] = &["toml.parse", "toml.stringify", "toml.toml"];

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
        let value: Value = toml::from_str(s)
            .map_err(|e| EvalError::Other(format!("toml parse error: {e}")))?;
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
    registry.register("toml.toml", Arc::new(TomlName));
    registry
}
/// to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy)]
pub struct TomlName;

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
