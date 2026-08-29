//! # handl-lib-toml
//!
//! TOML `Encoder`/`Decoder` for the `handl-lib-serialize` framework.
//!
//! Backed by `toml`.

use handl_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use serde_json::Value as JsonValue;
use toml::Value;

/// Combined TOML codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Toml;

/// TOML encoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct TomlEncoder;

/// TOML decoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct TomlDecoder;

impl Encoder<Value> for Toml {
    type Output = String;

    fn encode(&self, value: &Value) -> Result<Self::Output, SerializeError> {
        toml::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Decoder<str, Value> for Toml {
    fn decode(&self, input: &str) -> Result<Value, SerializeError> {
        toml::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Serializer for Toml {
    fn serialize(&self, value: &JsonValue) -> Result<String, SerializeError> {
        toml::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }

    fn deserialize(&self, input: &str) -> Result<JsonValue, SerializeError> {
        toml::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

use handl_plugin_vm::{Context, Function};
use handl_vm_common::Module;
use handl_vm_values_json::NonFunctionValue;

/// The `toml` module.
///
/// Exposes `toml` as a zero-arg function that returns the serializer name
/// so it can be passed to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TomlModule;

impl Module for TomlModule {
    fn name(&self) -> &str {
        "toml"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "toml" => Some(&TomlName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TomlName;

impl Function<NonFunctionValue> for TomlName {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_plugin_vm::Error<NonFunctionValue>> {
        if !args.is_empty() {
            return Err(handl_plugin_vm::Error::ArgumentCount {
                expected: 0,
                got: args.len(),
            });
        }
        Ok(NonFunctionValue::new(serde_json::json!({
            "type": "serializer",
            "name": "toml",
        })))
    }
}

impl Encoder<Value> for TomlEncoder {
    type Output = String;

    fn encode(&self, value: &Value) -> Result<Self::Output, SerializeError> {
        Toml.encode(value)
    }
}

impl Decoder<str, Value> for TomlDecoder {
    fn decode(&self, input: &str) -> Result<Value, SerializeError> {
        Toml.decode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
