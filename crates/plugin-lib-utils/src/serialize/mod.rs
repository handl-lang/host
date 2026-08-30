use std::sync::Arc;

use handl_plugin_lib_serialize::SerializerRegistry;
use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;

fn format_name(
    value: &serde_json::Value,
) -> Result<String, EvalError> {
    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }
    let obj = value
        .as_object()
        .ok_or_else(|| EvalError::TypeMismatch(format!("expected {}, got {}", "string or serializer object", "non-string, non-object",)))?;
    let ty = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        EvalError::TypeMismatch(format!("expected {}, got {}", "serializer object with type", "object without type",))
    })?;
    if ty != "serializer" {
        return Err(EvalError::TypeMismatch(format!("expected {}, got {}", "type: serializer", "non-serializer",)));
    }
    let name = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
        EvalError::TypeMismatch(format!("expected {}, got {}", "serializer object with name", "object without name",))
    })?;
    Ok(name.to_string())
}

/// The `serialize` module — generic serialization to/from strings.
///
/// Provides:
/// - `serialize.serialize(value, format)`
/// - `serialize.deserialize(text, format)`
pub struct SerializeModule {
    serialize: Serialize,
    deserialize: Deserialize,
}

impl SerializeModule {
    /// Creates a `SerializeModule` with the default serializers.
    pub fn new() -> Self {
        let registry = Arc::new(
            SerializerRegistry::new()
                .with("json", handl_plugin_lib_json::Json)
                .with("yaml", handl_plugin_lib_yaml::Yaml)
                .with("toml", handl_plugin_lib_toml::Toml),
        );
        Self {
            serialize: Serialize {
                registry: Arc::clone(&registry),
            },
            deserialize: Deserialize {
                registry: Arc::clone(&registry),
            },
        }
    }
}

impl Default for SerializeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SerializeModule {
    fn name(&self) -> &str {
        "serialize"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "serialize" => Some(&self.serialize),
            "deserialize" => Some(&self.deserialize),
            _ => None,
        }
    }
}

struct Serialize {
    registry: Arc<SerializerRegistry>,
}

impl Function for Serialize {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
        }
        let name = format_name(&args[1])?;
        let serializer = self.registry.get(&name).ok_or_else(|| {
            EvalError::Other(format!("unknown serializer: {name}"))
        })?;
        let output = serializer
            .serialize(&args[0])
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(serde_json::Value::from(output))
    }
}

struct Deserialize {
    registry: Arc<SerializerRegistry>,
}

impl Function for Deserialize {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
        }
        let name = format_name(&args[1])?;
        let serializer = self.registry.get(&name).ok_or_else(|| {
            EvalError::Other(format!("unknown serializer: {name}"))
        })?;
        let input = value_as_string(&args[0])?;
        let output = serializer
            .deserialize(&input)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(serde_json::Value::from(output))
    }
}

fn value_as_string(
    value: &serde_json::Value,
) -> Result<String, EvalError> {
    value
        .as_str()
        .map(String::from)
        .ok_or_else(|| EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))
}
