use handl_platform::{EvalContext, Function, Module, Plugin};
use handl_lib_bytes::ops::*;
use handl_plugin_lib_stdlib::{library, module};
use serde_json::json;

/// Build the canonical `{"__bytes__": [n, ...]}` value for tests.
fn bytes(items: &[i64]) -> serde_json::Value {
    json!({ "__bytes__": items })
}

#[test]
fn from_list_builds_bytes() {
    let func = FromList;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[json!([104, 105])], &mut ctx)
        .unwrap_or_else(|e| panic!("from_list failed: {e}"));
    assert_eq!(result, bytes(&[104, 105]));
}

#[test]
fn from_list_empty() {
    let func = FromList;
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!([])], &mut ctx).unwrap();
    assert_eq!(result, bytes(&[]));
}

#[test]
fn to_list_projects_bytes() {
    let func = ToList;
    let mut ctx = EvalContext::new();
    let result = func.call(&[bytes(&[0, 127, 255])], &mut ctx).unwrap();
    assert_eq!(result, json!([0, 127, 255]));
}

#[test]
fn from_list_to_list_roundtrip() {
    let from_list = FromList;
    let to_list = ToList;
    let mut ctx = EvalContext::new();
    let original = json!([0xDE, 0xAD, 0xBE, 0xEF]);
    let b = from_list.call(&[original.clone()], &mut ctx).unwrap();
    let back = to_list.call(&[b], &mut ctx).unwrap();
    assert_eq!(back, original);
}

#[test]
fn from_list_rejects_out_of_range() {
    let func = FromList;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!([256])], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("byte out of range 0..=255"));
    let err = func.call(&[json!([-1])], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("byte out of range 0..=255"));
}

#[test]
fn from_list_rejects_non_integer() {
    let func = FromList;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!([1.5])], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("integer"));
}

#[test]
fn from_list_rejects_non_array() {
    let func = FromList;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!("nope")], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("array"));
}

#[test]
fn to_list_rejects_non_bytes() {
    let func = ToList;
    let mut ctx = EvalContext::new();
    let err = func.call(&[json!([1, 2, 3])], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("Bytes"));
}

#[test]
fn concat_joins_buffers() {
    let func = Concat;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[bytes(&[0xDE, 0xAD]), bytes(&[0xBE, 0xEF])], &mut ctx)
        .unwrap();
    assert_eq!(result, bytes(&[0xDE, 0xAD, 0xBE, 0xEF]));
}

#[test]
fn concat_empty_is_identity() {
    let func = Concat;
    let mut ctx = EvalContext::new();
    let result = func.call(&[bytes(&[1]), bytes(&[])], &mut ctx).unwrap();
    assert_eq!(result, bytes(&[1]));
}

#[test]
fn len_returns_byte_count() {
    let func = Len;
    let mut ctx = EvalContext::new();
    let result = func.call(&[bytes(&[1, 2, 3])], &mut ctx).unwrap();
    assert_eq!(result, json!(3));
    let result = func.call(&[bytes(&[])], &mut ctx).unwrap();
    assert_eq!(result, json!(0));
}

#[test]
fn at_returns_byte() {
    let func = At;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[bytes(&[10, 20, 30]), json!(1)], &mut ctx)
        .unwrap();
    assert_eq!(result, json!(20));
}

#[test]
fn at_rejects_out_of_bounds() {
    let func = At;
    let mut ctx = EvalContext::new();
    let err = func
        .call(&[bytes(&[1, 2, 3]), json!(3)], &mut ctx)
        .unwrap_err();
    assert!(err.to_string().contains("out of bounds"));
}

#[test]
fn at_rejects_negative_index() {
    let func = At;
    let mut ctx = EvalContext::new();
    let err = func
        .call(&[bytes(&[1, 2, 3]), json!(-1)], &mut ctx)
        .unwrap_err();
    assert!(err.to_string().contains("non-negative"));
}

#[test]
fn slice_copies_window() {
    let func = Slice;
    let mut ctx = EvalContext::new();
    let result = func
        .call(&[bytes(&[1, 2, 3, 4, 5]), json!(1), json!(3)], &mut ctx)
        .unwrap();
    assert_eq!(result, bytes(&[2, 3, 4]));
}

#[test]
fn slice_clamps_to_buffer() {
    let func = Slice;
    let mut ctx = EvalContext::new();
    // Window extends past the end — clamps (matching str.slice).
    let result = func
        .call(&[bytes(&[1, 2, 3]), json!(1), json!(99)], &mut ctx)
        .unwrap();
    assert_eq!(result, bytes(&[2, 3]));
    // Offset past the end — empty.
    let result = func
        .call(&[bytes(&[1, 2, 3]), json!(9), json!(2)], &mut ctx)
        .unwrap();
    assert_eq!(result, bytes(&[]));
}

#[test]
fn slice_rejects_negative() {
    let func = Slice;
    let mut ctx = EvalContext::new();
    let err = func
        .call(&[bytes(&[1, 2, 3]), json!(-1), json!(2)], &mut ctx)
        .unwrap_err();
    assert!(err.to_string().contains("non-negative"));
    let err = func
        .call(&[bytes(&[1, 2, 3]), json!(0), json!(-2)], &mut ctx)
        .unwrap_err();
    assert!(err.to_string().contains("non-negative"));
}

#[test]
fn utf8_encode_produces_bytes() {
    let func = Utf8Encode;
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!("héllo")], &mut ctx).unwrap();
    assert_eq!(result, bytes(&[104, 0xC3, 0xA9, 108, 108, 111]));
    // Multi-byte char: "→" is E2 86 92.
    let result = func.call(&[json!("→")], &mut ctx).unwrap();
    assert_eq!(result, bytes(&[0xE2, 0x86, 0x92]));
}

#[test]
fn utf8_roundtrip() {
    let encode = Utf8Encode;
    let decode = Utf8Decode;
    let mut ctx = EvalContext::new();
    for s in ["héllo", "→", "", "ascii only"] {
        let b = encode.call(&[json!(s)], &mut ctx).unwrap();
        let back = decode.call(&[b], &mut ctx).unwrap();
        assert_eq!(back, json!(s));
    }
}

#[test]
fn utf8_decode_rejects_invalid() {
    let decode = Utf8Decode;
    let mut ctx = EvalContext::new();
    // 0xFF is never valid UTF-8.
    let err = decode.call(&[bytes(&[0xFF])], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("utf-8"));
    // Truncated multi-byte sequence (E2 86 without the third byte).
    let err = decode.call(&[bytes(&[0xE2, 0x86])], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("utf-8"));
}

#[test]
fn ops_reject_wrong_arg_types() {
    let mut ctx = EvalContext::new();
    let err = Concat
        .call(&[json!([1]), bytes(&[2])], &mut ctx)
        .unwrap_err();
    assert!(err.to_string().contains("Bytes"));
    let err = Utf8Encode.call(&[json!(42)], &mut ctx).unwrap_err();
    assert!(err.to_string().contains("string"));
}

#[test]
fn op_registry_registers_all_ops() {
    let registry = bytes_op_registry();
    for name in BYTES_OPS {
        assert!(registry.contains(name), "missing op {name}");
    }
    assert_eq!(registry.len(), BYTES_OPS.len());
}

#[test]
fn bytes_module_exposes_functions() {
    let module = module("bytes").unwrap();
    assert_eq!(module.name(), "bytes");

    let mut ctx = EvalContext::new();
    let from_list = module.function("from_list").expect("from_list");
    let b = from_list.call(&[json!([104, 105])], &mut ctx).unwrap();
    assert_eq!(b, bytes(&[104, 105]));

    let to_list = module.function("to_list").expect("to_list");
    let result = to_list.call(&[b], &mut ctx).unwrap();
    assert_eq!(result, json!([104, 105]));
}

#[test]
fn plugin_id_name_and_capabilities() {
    let plugin = library("bytes").unwrap();
    assert_eq!(plugin.id(), "lib.bytes");
    assert_eq!(plugin.name(), "lib-bytes");
    let caps = plugin.capabilities();
    assert_eq!(caps.len(), 8);
    assert!(caps.iter().any(|c| c.as_str() == "bytes.from_list"));
    assert!(caps.iter().any(|c| c.as_str() == "bytes.utf8_decode"));
}
