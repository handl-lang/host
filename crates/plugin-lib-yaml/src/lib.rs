//! # handl-lib-yaml
//!
//! YAML `Encoder`/`Decoder` for the `handl-lib-serialize` framework.
//!
//! Backed by `serde_yaml`.

use handl_plugin_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use serde_json::Value as JsonValue;
use serde_yaml::Value;

/// Combined YAML codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Yaml;

/// YAML encoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct YamlEncoder;

/// YAML decoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct YamlDecoder;

impl Encoder<Value> for Yaml {
    type Output = String;

    fn encode(&self, value: &Value) -> Result<Self::Output, SerializeError> {
        serde_yaml::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Decoder<str, Value> for Yaml {
    fn decode(&self, input: &str) -> Result<Value, SerializeError> {
        serde_yaml::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

impl Serializer for Yaml {
    fn serialize(&self, value: &JsonValue) -> Result<String, SerializeError> {
        serde_yaml::to_string(value).map_err(|e| SerializeError::new(e.to_string()))
    }

    fn deserialize(&self, input: &str) -> Result<JsonValue, SerializeError> {
        serde_yaml::from_str(input).map_err(|e| SerializeError::new(e.to_string()))
    }
}

use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;

/// The `yaml` module.
///
/// Exposes `yaml` as a zero-arg function that returns the serializer name
/// so it can be passed to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy, Default)]
pub struct YamlModule;

impl Module for YamlModule {
    fn name(&self) -> &str {
        "yaml"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "yaml" => Some(&YamlName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct YamlName;

impl Function for YamlName {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 0, args.len(),)));
        }
        Ok(serde_json::Value::from(serde_json::json!({
            "type": "serializer",
            "name": "yaml",
        })))
    }
}

impl Encoder<Value> for YamlEncoder {
    type Output = String;

    fn encode(&self, value: &Value) -> Result<Self::Output, SerializeError> {
        Yaml.encode(value)
    }
}

impl Decoder<str, Value> for YamlDecoder {
    fn decode(&self, input: &str) -> Result<Value, SerializeError> {
        Yaml.decode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

pub mod plugin;
pub use plugin::LibraryPlugin;
