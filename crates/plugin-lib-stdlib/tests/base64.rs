use handl_lib_base64::ops::*;
use handl_platform::{EvalContext, Function, Module, ModuleLookup, Plugin};
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{ModuleResolver, OpRegistry, RuntimePredicateChecker};
use handl_plugin_lib_stdlib::{PackageResolver, library, module, module_with};
use handl_plugin_module::ModuleRegistry;
use serde_json::{Value, json};
use std::sync::Arc;

/// Install the canonical default language once per test binary (see
/// `json.rs` — the `HandlModule` bridge runtimes resolve it lazily).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

/// Floor-module resolver: native `char` plus the package table — the same
/// resolver shape production passes to `module_with` (the flat `.hnl`
/// calls `char.*`; `char` has no table row by design, see `json.rs`).
fn floor_resolver() -> Arc<dyn ModuleResolver> {
    struct CharFloor;
    impl ModuleResolver for CharFloor {
        fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
            (name == "char").then(|| Arc::new(CharModule) as Arc<dyn Module + Send + Sync>)
        }
    }
    Arc::new(PackageResolver {
        fallback: Some(Arc::new(CharFloor)),
    })
}

/// The table module with the `char` floor wired, like production's
/// `module_with(name, StdlibResolver)` path.
fn table_module() -> handl_plugin_lib_handl::HandlModule {
    ensure_lang();
    module_with("base64", floor_resolver()).expect("base64 module in the package table")
}

#[test]
fn encode_works() {
    let func = Base64Encode;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!("hello")], &mut ctx)
        .unwrap_or_else(|e| panic!("encode failed: {e}"));
    assert_eq!(result, json!("aGVsbG8="));
}

#[test]
fn decode_works() {
    let func = Base64Decode;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!("aGVsbG8=")], &mut ctx)
        .unwrap_or_else(|e| panic!("decode failed: {e}"));
    assert_eq!(result, json!("hello"));
}

#[test]
fn encode_decode_roundtrip() {
    let encode = Base64Encode;
    let decode = Base64Decode;
    let mut ctx = EvalContext::new();
    let original = json!("Hello, World!");
    let encoded = encode.call(&[original.clone()], &mut ctx).unwrap();
    let decoded = decode.call(&[encoded], &mut ctx).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn op_registry_registers_both_ops() {
    let registry = base64_op_registry();
    assert!(registry.contains("base64.encode"));
    assert!(registry.contains("base64.decode"));
    assert_eq!(registry.len(), 3);
}

#[test]
fn base64_module_exposes_functions() {
    let module = table_module();
    assert_eq!(module.name(), "base64");

    let mut ctx = EvalContext::new();
    let encode = module.function("encode").expect("encode should exist");
    let result = encode.call(&[json!("test")], &mut ctx).unwrap();
    assert_eq!(result, json!("dGVzdA=="));

    let decode = module.function("decode").expect("decode should exist");
    let result = decode.call(&[json!("dGVzdA==")], &mut ctx).unwrap();
    assert_eq!(result, json!("test"));
}

/// The codec token (the successor of the old `base64` legacy name
/// function): `codec()` returns the self-describing carrier record.
#[test]
fn base64_module_codec_token() {
    let module = table_module();
    let mut ctx = EvalContext::new();
    let codec = module.function("codec").expect("codec should exist");
    let result = codec.call(&[], &mut ctx).unwrap();
    assert_eq!(result["type"], "codec");
    assert_eq!(result["name"], "base64");
    assert!(result.get("encode").is_some());
    assert!(result.get("decode").is_some());
}

#[test]
fn plugin_id_and_name() {
    let plugin = library("base64").unwrap();
    assert_eq!(plugin.id(), "lib.base64");
    assert_eq!(plugin.name(), "lib-base64");
}

#[test]
fn handl_plugin_id_and_name() {
    let plugin = library("base64").unwrap();
    assert_eq!(plugin.id(), "lib.base64");
    assert_eq!(plugin.name(), "lib-base64");
}

#[test]
fn base64_module_works_in_module_registry() {
    let module = table_module();
    let registry = ModuleRegistry::new().with(module);
    let lookup: &dyn ModuleLookup = &registry;

    let func = lookup
        .lookup("base64.encode")
        .expect("base64.encode should resolve");
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!("hello")], &mut ctx).unwrap();
    assert_eq!(result, json!("aGVsbG8="));
}

// -----------------------------------------------------------------------
// ADR-0077 Phase 3: runtime guarantee enforcement tests
// -----------------------------------------------------------------------

#[test]
fn base64_encode_guarantee_checker_validates_valid_output() {
    // The OpFunction dispatch path runs the guarantee checker. A valid
    // base64 output passes.
    let registry = Arc::new(base64_op_registry());
    let op_func = handl_plugin_lib_handl::OpFunction::new("base64.encode", registry);
    let mut ctx = EvalContext::new();
    let result = op_func.call(&[json!("hello")], &mut ctx).unwrap();
    assert_eq!(result, json!("aGVsbG8="));
}

#[test]
fn base64_encode_guarantee_checker_rejects_non_string_output() {
    // A checker directly: a non-string output is rejected.
    let checker = Base64GuaranteeChecker;
    let err = checker.check_guarantee(&json!(42)).unwrap_err();
    assert!(err.contains("expected a string output"));
}

#[test]
fn base64_encode_guarantee_checker_rejects_invalid_base64() {
    // A checker directly: a string that is not valid base64 is rejected.
    let checker = Base64GuaranteeChecker;
    let err = checker
        .check_guarantee(&json!("this is not base64!!!"))
        .unwrap_err();
    assert!(err.contains("IsBase64 violated"));
}

#[test]
fn base64_encode_guarantee_checker_accepts_valid_base64() {
    let checker = Base64GuaranteeChecker;
    checker.check_guarantee(&json!("aGVsbG8=")).unwrap();
    checker.check_guarantee(&json!("")).unwrap();
}

#[test]
fn base64_encode_op_rejects_when_guarantee_always_fails() {
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
        "base64.encode",
        Arc::new(Base64Encode),
        Some(Arc::new(AlwaysFailGuarantee)),
    );
    let shared = Arc::new(registry);
    let op_func = handl_plugin_lib_handl::OpFunction::new("base64.encode", shared);
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
fn base64_decode_op_has_no_checker_trusted() {
    // base64.decode is registered without a checker → axioms trusted,
    // behaves as before.
    let registry = Arc::new(base64_op_registry());
    assert!(registry.get_checker("base64.decode").is_none());
    assert!(registry.get_checker("base64.encode").is_some());

    let op_func = handl_plugin_lib_handl::OpFunction::new("base64.decode", registry);
    let mut ctx = EvalContext::new();
    let result = op_func.call(&[json!("aGVsbG8=")], &mut ctx).unwrap();
    assert_eq!(result, json!("hello"));
}

// -----------------------------------------------------------------------
// ADR-0077 Phase 5: capability declaration tests
// -----------------------------------------------------------------------

#[test]
fn base64_plugin_exposes_declared_capabilities() {
    let plugin = library("base64").unwrap();
    let caps = plugin.capabilities();
    assert_eq!(caps.len(), 3);
    assert!(caps.iter().any(|c| c.as_str() == "base64.encode"));
    assert!(caps.iter().any(|c| c.as_str() == "base64.decode"));
    assert!(caps.iter().any(|c| c.as_str() == "base64.codec"));
}

#[test]
fn base64_handl_plugin_exposes_declared_capabilities() {
    let plugin = library("base64").unwrap();
    let caps = plugin.capabilities();
    assert_eq!(caps.len(), 3);
    assert!(caps.iter().any(|c| c.as_str() == "base64.encode"));
    assert!(caps.iter().any(|c| c.as_str() == "base64.decode"));
    assert!(caps.iter().any(|c| c.as_str() == "base64.codec"));
}
