use std::sync::Arc;

use handl_plugin_lib_encoding::EncodingRegistry;
use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;

fn codec_name(
    value: &serde_json::Value,
) -> Result<String, EvalError> {
    let obj = value
        .as_object()
        .ok_or_else(|| EvalError::TypeMismatch(format!("expected {}, got {}", "codec object", "non-object",)))?;
    let ty = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        EvalError::TypeMismatch(format!("expected {}, got {}", "codec object with type", "object without type",))
    })?;
    if ty != "encoding" {
        return Err(EvalError::TypeMismatch(format!("expected {}, got {}", "type: encoding", "non-encoding",)));
    }
    let name = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
        EvalError::TypeMismatch(format!("expected {}, got {}", "codec object with name", "object without name",))
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

    fn function(&self, name: &str) -> Option<&dyn Function> {
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

impl Function for Encode {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
        }
        let name = codec_name(&args[1])?;
        let encoding = self.registry.get(&name).ok_or_else(|| {
            EvalError::Other(format!("unknown encoding: {name}"))
        })?;
        let output = encoding
            .encode(&args[0])
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(serde_json::Value::from(output))
    }
}

struct Decode {
    registry: Arc<EncodingRegistry>,
}

impl Function for Decode {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
        }
        let name = codec_name(&args[1])?;
        let encoding = self.registry.get(&name).ok_or_else(|| {
            EvalError::Other(format!("unknown encoding: {name}"))
        })?;
        let output = encoding
            .decode(&args[0])
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(serde_json::Value::from(output))
    }
}
