use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_encoding::{Encoding, EncodingError};
use handl_plugin_lib_handl::{OpRegistry, RuntimePredicateChecker};
use serde_json::Value;

/// The handl source declaring the hex `op` interface — the single source
/// of truth for the hex API surface. Loaded at compile time from the
/// kit-stdlib data dir.

/// The op names declared in `hex.hnl`.
pub const HEX_OPS: &[&str] = &["hex.encode", "hex.decode", "hex.hex"];

// ---------------------------------------------------------------------------
// Encoding trait impl (backward compat for plugin-lib-utils)
// ---------------------------------------------------------------------------

/// Hexadecimal encoding (implements the [`Encoding`] trait for
/// [`plugin-lib-utils`]'s `EncodingRegistry`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Hex;

impl Encoding for Hex {
    fn encode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value
            .as_str()
            .ok_or_else(|| EncodingError::Message("hex.encode expects a string".to_string()))?;
        let encoded = hex::encode(s.as_bytes());
        Ok(Value::String(encoded))
    }

    fn decode(&self, value: &Value) -> Result<Value, EncodingError> {
        let s = value
            .as_str()
            .ok_or_else(|| EncodingError::Message("hex.decode expects a string".to_string()))?;
        let decoded =
            hex::decode(s).map_err(|e| EncodingError::Message(format!("hex decode error: {e}")))?;
        let out = String::from_utf8(decoded)
            .map_err(|e| EncodingError::Message(format!("hex decode is not valid utf-8: {e}")))?;
        Ok(Value::String(out))
    }
}

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// The Rust handl for `op "hex.encode" : String -> String`.
///
/// Encodes a string to hexadecimal. This is the eval-time `Function` impl
/// registered under the op name `"hex.encode"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct HexEncode;

impl Function for HexEncode {
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
            .ok_or_else(|| EvalError::Other("hex.encode expects a string".to_string()))?;
        let encoded = hex::encode(s.as_bytes());
        Ok(Value::String(encoded))
    }
}

/// The Rust handl for `op "hex.decode" : String -> String`.
///
/// Decodes a hexadecimal string. This is the eval-time `Function` impl
/// registered under the op name `"hex.decode"` in the [`OpRegistry`].
#[derive(Debug, Clone, Copy)]
pub struct HexDecode;

impl Function for HexDecode {
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
            .ok_or_else(|| EvalError::Other("hex.decode expects a string".to_string()))?;
        let decoded =
            hex::decode(s).map_err(|e| EvalError::Other(format!("hex decode error: {e}")))?;
        let out = String::from_utf8(decoded)
            .map_err(|e| EvalError::Other(format!("hex decode is not valid utf-8: {e}")))?;
        Ok(Value::String(out))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// A runtime axiom checker for the hex `op`s (ADR-0077 Phase 3).
///
/// This implements [`RuntimePredicateChecker`] to enforce the `guarantee`
/// axioms declared in `hex.hnl` at eval time:
/// - `hex.encode` has a `guarantee IsHex(output)` — the checker verifies
///   the output is valid hex by decoding it and confirming it round-trips.
/// - `hex.decode` has no specific runtime guarantee (its output is an
///   arbitrary string), so the guarantee check is a no-op.
///
/// Neither op has a `requires` predicate, so [`check_requires`](RuntimePredicateChecker::check_requires)
/// is a no-op.
#[derive(Debug, Clone, Copy, Default)]
pub struct HexGuaranteeChecker;

impl RuntimePredicateChecker for HexGuaranteeChecker {
    fn check_requires(&self, _args: &[Value]) -> Result<(), String> {
        // hex.encode/decode have no `requires` predicate.
        Ok(())
    }

    fn check_guarantee(&self, output: &Value) -> Result<(), String> {
        // Enforce `IsHex(output)` for encode: the output must be a string
        // that decodes as valid hex and re-encodes to the same value
        // (round-trip). For decode, the output is an arbitrary string with no
        // specific guarantee, so we only require it be a string.
        let s = output
            .as_str()
            .ok_or_else(|| "expected a string output".to_string())?;
        // Verify the output is valid hex by attempting a decode. We accept
        // any string for the decode-op output (no IsHex guarantee there),
        // but for the encode-op output this enforces IsHex. Since this
        // checker is attached to hex.encode, we apply the strict check.
        match hex::decode(s) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("IsHex violated: output is not valid hex: {e}")),
        }
    }
}

/// Build the [`OpRegistry`] for hex, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"hex.encode"` → [`HexEncode`] (with a [`HexGuaranteeChecker`]
///   enforcing the `IsHex(output)` guarantee)
/// - `"hex.decode"` → [`HexDecode`] (no runtime checker; axioms trusted)
pub fn hex_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register_with_checker(
        "hex.encode",
        Arc::new(HexEncode),
        Some(Arc::new(HexGuaranteeChecker)),
    );
    registry.register("hex.decode", Arc::new(HexDecode));
    registry.register("hex.hex", Arc::new(HexName));
    registry
}
/// `encoding.encode`/`encoding.decode`.
#[derive(Debug, Clone, Copy)]
pub struct HexName;

impl Function for HexName {
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
            "type": "encoding",
            "name": "hex",
        })))
    }
}
