use std::sync::Arc;

use handl_lib_serialize::SerializerRegistry;
use handl_vm::{Context, Function};
use handl_vm_common::Module;
use handl_vm_values_json::NonFunctionValue;

fn format_name(value: &NonFunctionValue) -> Result<String, handl_vm::Error<NonFunctionValue>> {
    value.as_str().map(String::from).ok_or_else(|| {
        handl_vm::Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        }
    })
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
                .with("json", handl_lib_json::Json)
                .with("yaml", handl_lib_yaml::Yaml)
                .with("toml", handl_lib_toml::Toml),
        );
        Self {
            serialize: Serialize { registry: Arc::clone(&registry) },
            deserialize: Deserialize { registry: Arc::clone(&registry) },
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

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
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

impl Function<NonFunctionValue> for Serialize {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_vm::Error<NonFunctionValue>> {
        if args.len() != 2 {
            return Err(handl_vm::Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }
        let name = format_name(&args[1])?;
        let serializer = self
            .registry
            .get(&name)
            .ok_or_else(|| handl_vm::Error::PrimitiveError(format!("unknown serializer: {name}")))?;
        let output = serializer
            .serialize(args[0].inner())
            .map_err(|e| handl_vm::Error::PrimitiveError(e.to_string()))?;
        Ok(NonFunctionValue::new(output))
    }
}

struct Deserialize {
    registry: Arc<SerializerRegistry>,
}

impl Function<NonFunctionValue> for Deserialize {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_vm::Error<NonFunctionValue>> {
        if args.len() != 2 {
            return Err(handl_vm::Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }
        let name = format_name(&args[1])?;
        let serializer = self
            .registry
            .get(&name)
            .ok_or_else(|| handl_vm::Error::PrimitiveError(format!("unknown serializer: {name}")))?;
        let input = format_name(&args[0])?;
        let output = serializer
            .deserialize(&input)
            .map_err(|e| handl_vm::Error::PrimitiveError(e.to_string()))?;
        Ok(NonFunctionValue::new(output))
    }
}
