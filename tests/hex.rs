use std::sync::Arc;
use handl_plugin_lib_stdlib::ops::hex::*;
use handl_plugin_lib_handl::{OpRegistry, RuntimePredicateChecker};
use handl_plugin_lib_stdlib::{module, module_ref, library};
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup, Plugin};
use handl_plugin_module::ModuleRegistry;
use serde_json::{json, Value};

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
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn hex_module_exposes_functions() {
        let module = module("hex").unwrap();
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
        let module = module("hex").unwrap();
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
        let plugin = library("hex").unwrap();
        assert_eq!(plugin.id(), "lib.hex");
        assert_eq!(plugin.name(), "lib-hex");
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = library("hex").unwrap();
        assert_eq!(plugin.id(), "lib.hex");
        assert_eq!(plugin.name(), "lib-hex");
    }

    #[test]
    fn hex_module_works_in_module_registry() {
        let module = module("hex").unwrap();
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
        let plugin = library("hex").unwrap();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].as_str(), "hex.encode");
        assert_eq!(caps[1].as_str(), "hex.decode");
    }

    #[test]
    fn hex_handl_plugin_exposes_declared_capabilities() {
        let plugin = library("hex").unwrap();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].as_str(), "hex.encode");
        assert_eq!(caps[1].as_str(), "hex.decode");
    }
