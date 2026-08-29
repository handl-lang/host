use std::sync::Arc;

use handl_plugin_lib_encoding::EncodingRegistry;
use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::NonFunctionValue;

fn codec_name(
    value: &NonFunctionValue,
) -> Result<String, handl_plugin_vm::Error<NonFunctionValue>> {
    let obj = value
        .as_object()
        .ok_or_else(|| handl_plugin_vm::Error::TypeMismatch {
            expected: "codec object",
            got: "non-object",
        })?;
    let ty = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        handl_plugin_vm::Error::TypeMismatch {
            expected: "codec object with type",
            got: "object without type",
        }
    })?;
    if ty != "encoding" {
        return Err(handl_plugin_vm::Error::TypeMismatch {
            expected: "type: encoding",
            got: "non-encoding",
        });
    }
    let name = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
        handl_plugin_vm::Error::TypeMismatch {
            expected: "codec object with name",
            got: "object without name",
        }
    })?;
    Ok(name.to_string())
}

/// The `encoding` module — generic encoding to/from string forms.
///
/// Provides:
/// - `encoding.encode(value, codec)`
/// - `encoding.decode(value, codec)`
pub struct EncodingModule {
    encode: Encode,
    decode: Decode,
}

impl EncodingModule {
    /// Creates an `EncodingModule` with the default encodings.
    pub fn new() -> Self {
        let registry = Arc::new(
            EncodingRegistry::new()
                .with("base64", handl_plugin_lib_base64::Base64)
                .with("hex", handl_plugin_lib_hex::Hex),
        );
        Self {
            encode: Encode {
                registry: Arc::clone(&registry),
            },
            decode: Decode {
                registry: Arc::clone(&registry),
            },
        }
    }
}

impl Default for EncodingModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for EncodingModule {
    fn name(&self) -> &str {
        "encoding"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "encode" => Some(&self.encode),
            "decode" => Some(&self.decode),
            _ => None,
        }
    }
}

struct Encode {
    registry: Arc<EncodingRegistry>,
}

impl Function<NonFunctionValue> for Encode {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_plugin_vm::Error<NonFunctionValue>> {
        if args.len() != 2 {
            return Err(handl_plugin_vm::Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }
        let name = codec_name(&args[1])?;
        let encoding = self.registry.get(&name).ok_or_else(|| {
            handl_plugin_vm::Error::PrimitiveError(format!("unknown encoding: {name}"))
        })?;
        let output = encoding
            .encode(args[0].inner())
            .map_err(|e| handl_plugin_vm::Error::PrimitiveError(e.to_string()))?;
        Ok(NonFunctionValue::new(output))
    }
}

struct Decode {
    registry: Arc<EncodingRegistry>,
}

impl Function<NonFunctionValue> for Decode {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_plugin_vm::Error<NonFunctionValue>> {
        if args.len() != 2 {
            return Err(handl_plugin_vm::Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }
        let name = codec_name(&args[1])?;
        let encoding = self.registry.get(&name).ok_or_else(|| {
            handl_plugin_vm::Error::PrimitiveError(format!("unknown encoding: {name}"))
        })?;
        let output = encoding
            .decode(args[0].inner())
            .map_err(|e| handl_plugin_vm::Error::PrimitiveError(e.to_string()))?;
        Ok(NonFunctionValue::new(output))
    }
}
