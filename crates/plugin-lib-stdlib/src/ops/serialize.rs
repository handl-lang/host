//! Rust `Function` impls for the `serialize` module — the handls (handlers)
//! for the `op`/`handl` declarations in `serialize.hnl`.
//!
//! Both functions are FFI atoms: they delegate to a `SerializerRegistry`
//! (from `plugin-lib-serialize`) wrapping Json/Yaml/Toml serializers and
//! cannot be expressed as pure handl arrows without losing the exact error
//! messages the tests rely on. The `op`/`handl` declaration form
//! (ADR-0077 Phase 2) declares the interface in `serialize.hnl`; these
//! impls provide the logic and are registered in an
//! [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).

use std::sync::Arc;

use handl_plugin_lib_handl::OpRegistry;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_serialize::SerializerRegistry;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_name(value: &serde_json::Value) -> Result<String, EvalError> {
    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }
    let obj = value.as_object().ok_or_else(|| {
        EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string or serializer object", "non-string, non-object",
        ))
    })?;
    let ty = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "serializer object with type", "object without type",
        ))
    })?;
    if ty != "serializer" {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "type: serializer", "non-serializer",
        )));
    }
    let name = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
        EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "serializer object with name", "object without name",
        ))
    })?;
    Ok(name.to_string())
}

fn value_as_string(value: &serde_json::Value) -> Result<String, EvalError> {
    value.as_str().map(String::from).ok_or_else(|| {
        EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",))
    })
}

// ---------------------------------------------------------------------------
// Serialize / Deserialize
// ---------------------------------------------------------------------------

/// The `serialize.serialize` handl — serialize a value into a string using a
/// named format (e.g. `"json"`, `"yaml"`, `"toml"`).
pub struct Serialize {
    pub(crate) registry: Arc<SerializerRegistry>,
}

impl Function for Serialize {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let name = format_name(&args[1])?;
        let serializer = self
            .registry
            .get(&name)
            .ok_or_else(|| EvalError::Other(format!("unknown serializer: {name}")))?;
        let output = serializer
            .serialize(&args[0])
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(serde_json::Value::from(output))
    }
}

/// The `serialize.deserialize` handl — deserialize a string into a value
/// using a named format (e.g. `"json"`, `"yaml"`, `"toml"`).
pub struct Deserialize {
    pub(crate) registry: Arc<SerializerRegistry>,
}

impl Function for Deserialize {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let name = format_name(&args[1])?;
        let serializer = self
            .registry
            .get(&name)
            .ok_or_else(|| EvalError::Other(format!("unknown serializer: {name}")))?;
        let input = value_as_string(&args[0])?;
        let output = serializer
            .deserialize(&input)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(serde_json::Value::from(output))
    }
}

pub const SERIALIZE_OPS: &[&str] = &["serialize.serialize", "serialize.deserialize"];

pub fn serialize_op_registry() -> OpRegistry {
    let registry = Arc::new(SerializerRegistry::new().with("json", crate::ops::json::Json));
    let mut registry_ops = OpRegistry::new();
    registry_ops.register(
        "serialize.serialize",
        Arc::new(Serialize {
            registry: Arc::clone(&registry),
        }),
    );
    registry_ops.register(
        "serialize.deserialize",
        Arc::new(Deserialize {
            registry: Arc::clone(&registry),
        }),
    );
    registry_ops
}
