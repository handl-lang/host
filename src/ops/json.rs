use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use handl_plugin_lib_serialize::{Decoder, Encoder, SerializeError, Serializer};
use serde_json::Value;

/// The handl source declaring the json `op` interface — the single source
/// of truth for the json API surface. Loaded at compile time from the
/// kit-stdlib data dir.

/// The op names declared in `json.hnl`.
pub const JSON_OPS: &[&str] = &["json.parse", "json.stringify", "json.json"];

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
    registry.register("json.json", Arc::new(JsonName));
    registry
}
/// to `serialize.serialize`/`serialize.deserialize`.
#[derive(Debug, Clone, Copy)]
pub struct JsonName;

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
