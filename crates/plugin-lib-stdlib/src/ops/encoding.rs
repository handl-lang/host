//! Rust `Function` impls for the `encoding` module — the handls (handlers)
//! for the `op`/`handl` declarations in `encoding.hnl`.
//!
//! Both functions are FFI atoms: they delegate to an `EncodingRegistry`
//! (from `plugin-lib-encoding`) wrapping Base64 and Hex codecs and cannot
//! be expressed as pure handl arrows without losing the exact error messages
//! the tests rely on. The `op`/`handl` declaration form (ADR-0077 Phase 2)
//! declares the interface in `encoding.hnl`; these impls provide the logic
//! and are registered in an
//! [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).

use std::sync::Arc;

use handl_plugin_lib_handl::OpRegistry;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_encoding::EncodingRegistry;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn codec_name(value: &serde_json::Value) -> Result<String, EvalError> {
    let obj = value.as_object().ok_or_else(|| {
        EvalError::TypeMismatch(format!("expected {}, got {}", "codec object", "non-object",))
    })?;
    let ty = obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "codec object with type", "object without type",
        ))
    })?;
    if ty != "encoding" {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "type: encoding", "non-encoding",
        )));
    }
    let name = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
        EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "codec object with name", "object without name",
        ))
    })?;
    Ok(name.to_string())
}

// ---------------------------------------------------------------------------
// Encode / Decode
// ---------------------------------------------------------------------------

/// The `encoding.encode` handl — encode a value into a string form using a
/// named codec (e.g. `"base64"`, `"hex"`).
pub struct Encode {
    pub(crate) registry: Arc<EncodingRegistry>,
}

impl Function for Encode {
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
        let name = codec_name(&args[1])?;
        let encoding = self
            .registry
            .get(&name)
            .ok_or_else(|| EvalError::Other(format!("unknown encoding: {name}")))?;
        let output = encoding
            .encode(&args[0])
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(serde_json::Value::from(output))
    }
}

/// The `encoding.decode` handl — decode a value into a value using a named
/// codec (e.g. `"base64"`, `"hex"`).
pub struct Decode {
    pub(crate) registry: Arc<EncodingRegistry>,
}

impl Function for Decode {
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
        let name = codec_name(&args[1])?;
        let encoding = self
            .registry
            .get(&name)
            .ok_or_else(|| EvalError::Other(format!("unknown encoding: {name}")))?;
        let output = encoding
            .decode(&args[0])
            .map_err(|e| EvalError::Other(e.to_string()))?;
        Ok(serde_json::Value::from(output))
    }
}

pub const ENCODING_OPS: &[&str] = &["encoding.encode", "encoding.decode"];

pub fn encoding_op_registry() -> OpRegistry {
    let registry = Arc::new(
        EncodingRegistry::new()
            .with("base64", crate::ops::base64::Base64)
            .with("hex", crate::ops::hex::Hex),
    );
    let mut registry_ops = OpRegistry::new();
    registry_ops.register(
        "encoding.encode",
        Arc::new(Encode {
            registry: Arc::clone(&registry),
        }),
    );
    registry_ops.register(
        "encoding.decode",
        Arc::new(Decode {
            registry: Arc::clone(&registry),
        }),
    );
    registry_ops
}
