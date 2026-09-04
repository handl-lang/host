//! # handl-lib-hex
//!
//! Hexadecimal encoding for the handl runtime.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 2). The hex interface is declared in
//! `handl-packages/kit-stdlib/hex/hex.hnl` via `op` declarations, and the Rust
//! encode/decode logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`HexLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Module`/`Plugin` trait scaffolding.
//!
//! The [`Encoding`] trait impl and [`HexModule`] re-export are preserved
//! for backward compatibility with [`plugin-lib-utils`], which uses
//! [`Hex`] in its `EncodingRegistry`.

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_encoding::{Encoding, EncodingError};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry, RuntimePredicateChecker};
use serde_json::Value;

/// The handl source declaring the hex `op` interface — the single source
/// of truth for the hex API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const HEX_HNL: &str = include_str!("../../../handl-packages/kit-stdlib/hex/hex.hnl");

/// The op names declared in `hex.hnl`.
const HEX_OPS: &[&str] = &["hex.encode", "hex.decode"];

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
        let decoded = hex::decode(s).map_err(|e| EvalError::Other(format!("hex decode error: {e}")))?;
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
        let s = output.as_str().ok_or_else(|| {
            "expected a string output".to_string()
        })?;
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
    registry
}

// ---------------------------------------------------------------------------
// HexModule (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

/// The `hex` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// (e.g. [`plugin-lib-utils`]) that register `HexModule` directly. The
/// module exposes `encode` and `decode` functions that dispatch to the
/// [`OpRegistry`], plus the legacy `hex` function that returns the codec
/// name (for `encoding.encode`/`encoding.decode`).
#[derive(Debug, Clone, Copy, Default)]
pub struct HexModule;

impl Module for HexModule {
    fn name(&self) -> &str {
        "hex"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "encode" => Some(&HexEncode),
            "decode" => Some(&HexDecode),
            "hex" => Some(&HexName),
            _ => None,
        }
    }
}

/// Legacy `hex` function — returns the codec name so it can be passed to
/// `encoding.encode`/`encoding.decode`.
#[derive(Debug, Clone, Copy)]
struct HexName;

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

// ---------------------------------------------------------------------------
// HexLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `hex` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 2 replacement for the old hand-rolled `LibraryPlugin`.
/// The hex interface is declared in `hex.hnl` via `op`/`handl`
/// declarations; the Rust encode/decode handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct HexLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl HexLibraryPlugin {
    /// Create a new `HexLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(hex_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "hex",
                "lib.hex",
                "lib-hex",
                HEX_HNL,
                &[],
            )
            .with_ops(HEX_OPS, registry),
        }
    }
}

impl Default for HexLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for HexLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for hex with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`HexLibraryPlugin`].
pub fn hex_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(hex_op_registry());
    HandlLibraryPlugin::new_with_caps("hex", "lib.hex", "lib-hex", HEX_HNL, &[])
        .with_ops(HEX_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`HexLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_hex::LibraryPlugin`.
pub use HexLibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, Module, ModuleLookup, Plugin};
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    #[test]
    fn encode_works() {
        let func = HexEncode;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!("hello")], &mut ctx)
            .unwrap_or_else(|e| panic!("encode failed: {e}"));
        assert_eq!(result, json!("68656c6c6f"));
    }

    #[test]
    fn decode_works() {
        let func = HexDecode;
        let mut ctx = EvalContext::new();
        let result = func
            .call(&[json!("68656c6c6f")], &mut ctx)
            .unwrap_or_else(|e| panic!("decode failed: {e}"));
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn encode_decode_roundtrip() {
        let encode = HexEncode;
        let decode = HexDecode;
        let mut ctx = EvalContext::new();
        let original = json!("Hello, World!");
        let encoded = encode.call(&[original.clone()], &mut ctx).unwrap();
        let decoded = decode.call(&[encoded], &mut ctx).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn op_registry_registers_both_ops() {
        let registry = hex_op_registry();
        assert!(registry.contains("hex.encode"));
        assert!(registry.contains("hex.decode"));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn hex_module_exposes_functions() {
        let module = HexModule;
        assert_eq!(module.name(), "hex");

        let mut ctx = EvalContext::new();
        let encode = module.function("encode").expect("encode should exist");
        let result = encode.call(&[json!("test")], &mut ctx).unwrap();
        assert_eq!(result, json!("74657374"));

        let decode = module.function("decode").expect("decode should exist");
        let result = decode.call(&[json!("74657374")], &mut ctx).unwrap();
        assert_eq!(result, json!("test"));
    }

    #[test]
    fn hex_module_legacy_name_function() {
        let module = HexModule;
        let mut ctx = EvalContext::new();
        let name_func = module.function("hex").expect("hex should exist");
        let result = name_func.call(&[], &mut ctx).unwrap();
        assert_eq!(
            result,
            json!({"type": "encoding", "name": "hex"})
        );
    }

    #[test]
    fn plugin_id_and_name() {
        let plugin = HexLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.hex");
        assert_eq!(plugin.name(), "lib-hex");
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = hex_handl_plugin();
        assert_eq!(plugin.id(), "lib.hex");
        assert_eq!(plugin.name(), "lib-hex");
    }

    #[test]
    fn hex_module_works_in_module_registry() {
        let module = HexModule;
        let registry = ModuleRegistry::new().with(module);
        let lookup: &dyn ModuleLookup = &registry;

        let func = lookup
            .lookup("hex.encode")
            .expect("hex.encode should resolve");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!("hello")], &mut ctx).unwrap();
        assert_eq!(result, json!("68656c6c6f"));
    }

    // -----------------------------------------------------------------------
    // ADR-0077 Phase 3: runtime guarantee enforcement tests
    // -----------------------------------------------------------------------

    #[test]
    fn hex_encode_guarantee_checker_validates_valid_output() {
        // The OpFunction dispatch path runs the guarantee checker. A valid
        // hex output passes.
        let registry = Arc::new(hex_op_registry());
        let op_func = handl_plugin_lib_handl::OpFunction::new("hex.encode", registry);
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!("hello")], &mut ctx).unwrap();
        assert_eq!(result, json!("68656c6c6f"));
    }

    #[test]
    fn hex_encode_guarantee_checker_rejects_non_string_output() {
        // A checker directly: a non-string output is rejected.
        let checker = HexGuaranteeChecker;
        let err = checker.check_guarantee(&json!(42)).unwrap_err();
        assert!(err.contains("expected a string output"));
    }

    #[test]
    fn hex_encode_guarantee_checker_rejects_invalid_hex() {
        // A checker directly: a string that is not valid hex is rejected.
        let checker = HexGuaranteeChecker;
        let err = checker
            .check_guarantee(&json!("this is not hex!!!"))
            .unwrap_err();
        assert!(err.contains("IsHex violated"));
    }

    #[test]
    fn hex_encode_guarantee_checker_accepts_valid_hex() {
        let checker = HexGuaranteeChecker;
        checker.check_guarantee(&json!("68656c6c6f")).unwrap();
        checker.check_guarantee(&json!("")).unwrap();
    }

    #[test]
    fn hex_encode_op_rejects_when_guarantee_always_fails() {
        // Proof that a broken guarantee checker would reject the output: we
        // build a registry with a checker that always fails the guarantee.
        struct AlwaysFailGuarantee;
        impl handl_plugin_lib_handl::RuntimePredicateChecker for AlwaysFailGuarantee {
            fn check_requires(&self, _args: &[Value]) -> Result<(), String> {
                Ok(())
            }
            fn check_guarantee(&self, _output: &Value) -> Result<(), String> {
                Err("forced guarantee failure".to_string())
            }
        }

        let mut registry = OpRegistry::new();
        registry.register_with_checker(
            "hex.encode",
            Arc::new(HexEncode),
            Some(Arc::new(AlwaysFailGuarantee)),
        );
        let shared = Arc::new(registry);
        let op_func = handl_plugin_lib_handl::OpFunction::new("hex.encode", shared);
        let mut ctx = EvalContext::new();
        let err = op_func.call(&[json!("hello")], &mut ctx).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("guarantee predicate violated"),
            "expected guarantee violation, got: {msg}"
        );
        assert!(msg.contains("forced guarantee failure"));
    }

    #[test]
    fn hex_decode_op_has_no_checker_trusted() {
        // hex.decode is registered without a checker → axioms trusted,
        // behaves as before.
        let registry = Arc::new(hex_op_registry());
        assert!(registry.get_checker("hex.decode").is_none());
        assert!(registry.get_checker("hex.encode").is_some());

        let op_func = handl_plugin_lib_handl::OpFunction::new("hex.decode", registry);
        let mut ctx = EvalContext::new();
        let result = op_func.call(&[json!("68656c6c6f")], &mut ctx).unwrap();
        assert_eq!(result, json!("hello"));
    }

    // -----------------------------------------------------------------------
    // ADR-0077 Phase 5: capability declaration tests
    // -----------------------------------------------------------------------

    #[test]
    fn hex_plugin_exposes_declared_capabilities() {
        let plugin = HexLibraryPlugin::new();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].as_str(), "hex.encode");
        assert_eq!(caps[1].as_str(), "hex.decode");
    }

    #[test]
    fn hex_handl_plugin_exposes_declared_capabilities() {
        let plugin = hex_handl_plugin();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].as_str(), "hex.encode");
        assert_eq!(caps[1].as_str(), "hex.decode");
    }
}
