//! # handl-lib-json
//!
//! JSON `Encoder`/`Decoder` for the `handl-lib-serialize` framework.
//!
//! Backed by `serde_json`.

use handl_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use serde_json::Value;

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

use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::NonFunctionValue;

/// The `json` module.
///
/// Exposes `json` as a zero-arg function that returns the serializer name
/// so it can be passed to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonModule;

impl Module for JsonModule {
    fn name(&self) -> &str {
        "json"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "json" => Some(&JsonName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct JsonName;

impl Function<NonFunctionValue> for JsonName {
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
            "name": "json",
        })))
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

pub mod plugin;
pub use plugin::LibraryPlugin;
