//! Tests for the FFI dispatcher using real Linux syscalls.
//!
//! All tests are `#[cfg(target_os = "linux")]` because they call into libc
//! directly via `dlopen` / `dlsym` and rely on glibc-specific symbols
//! (`getpid`, `abs`, `labs`, `fabs`, `strlen`, `open`) and errno semantics
//! (`__errno_location`, `ENOENT = 2`).

#![cfg(target_os = "linux")]

use handl_platform::{EvalContext, EvalError, Function};
use serde_json::{Value, json};

use crate::marshal::{CArg, CRet, marshal_arg, unmarshal_return};
use crate::FfiOpFunction;

/// Call an [`FfiOpFunction`] with a fresh [`EvalContext`].
fn call(func: &FfiOpFunction, args: &[Value]) -> Result<Value, EvalError> {
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
}

// -------------------------------------------------------------------------
// getpid — no args, returns i32
// -------------------------------------------------------------------------

#[test]
fn getpid_returns_current_pid() {
    let func = FfiOpFunction::new("libc.so.6", "getpid", vec![], "i32", false);
    let result = call(&func, &[]).expect("getpid call should succeed");
    let pid = result
        .as_i64()
        .expect("getpid should return an integer") as u32;
    assert_eq!(pid, std::process::id());
}

// -------------------------------------------------------------------------
// Marshalling round-trips via real libc functions
// -------------------------------------------------------------------------

#[test]
fn marshal_i32_roundtrip() {
    // abs(-42) = 42
    let func = FfiOpFunction::new(
        "libc.so.6",
        "abs",
        vec!["i32".to_string()],
        "i32",
        false,
    );
    let result = call(&func, &[json!(-42)]).expect("abs call should succeed");
    assert_eq!(result.as_i64(), Some(42));
}

#[test]
fn marshal_i64_roundtrip() {
    // labs(long) on 64-bit Linux: long is 64-bit.
    let val: i64 = -9_223_372_036_854_775_807; // i64::MIN + 1
    let func = FfiOpFunction::new(
        "libc.so.6",
        "labs",
        vec!["i64".to_string()],
        "i64",
        false,
    );
    let result = call(&func, &[json!(val)]).expect("labs call should succeed");
    assert_eq!(result.as_i64(), Some(val.abs()));
}

#[test]
fn marshal_f64_roundtrip() {
    // fabs(-42.5) = 42.5 — fabs lives in libm, not libc.
    let func = FfiOpFunction::new(
        "libm.so.6",
        "fabs",
        vec!["f64".to_string()],
        "f64",
        false,
    );
    let result = call(&func, &[json!(-42.5)]).expect("fabs call should succeed");
    let f = result
        .as_f64()
        .expect("fabs should return a number");
    assert!((f - 42.5).abs() < f64::EPSILON);
}

#[test]
fn marshal_cstr_roundtrip() {
    // strlen("hello") = 5 — verifies the cstr arg is correctly null-terminated
    // and passed as a pointer.
    let func = FfiOpFunction::new(
        "libc.so.6",
        "strlen",
        vec!["cstr".to_string()],
        "i64",
        false,
    );
    let result = call(&func, &[json!("hello")]).expect("strlen call should succeed");
    assert_eq!(result.as_i64(), Some(5));
}

#[test]
fn marshal_cstr_return_unmarshal() {
    // Direct unit test: unmarshal a CRet::Int holding a valid cstr pointer.
    let c = std::ffi::CString::new("hello").unwrap();
    let ptr = c.as_ptr() as i64;
    let ret = CRet::Int(ptr);
    let v = unmarshal_return("cstr", &ret).expect("cstr unmarshal");
    assert_eq!(v, json!("hello"));

    // Null pointer → JSON null.
    let ret = CRet::Int(0);
    let v = unmarshal_return("cstr", &ret).expect("cstr null unmarshal");
    assert_eq!(v, Value::Null);
}

// -------------------------------------------------------------------------
// Error cases
// -------------------------------------------------------------------------

#[test]
fn error_on_i32_overflow() {
    // 2^31 exceeds i32 range — marshal should fail before the call.
    let func = FfiOpFunction::new(
        "libc.so.6",
        "abs",
        vec!["i32".to_string()],
        "i32",
        false,
    );
    let result = call(&func, &[json!(2_147_483_648_i64)]); // 2^31
    assert!(
        matches!(result, Err(EvalError::TypeMismatch(_))),
        "expected TypeMismatch for i32 overflow, got {result:?}"
    );
}

#[test]
fn error_on_embedded_nul_in_cstr() {
    let func = FfiOpFunction::new(
        "libc.so.6",
        "strlen",
        vec!["cstr".to_string()],
        "i64",
        false,
    );
    let result = call(&func, &[json!("hello\0world")]);
    assert!(
        matches!(result, Err(EvalError::TypeMismatch(_))),
        "expected TypeMismatch for embedded NUL, got {result:?}"
    );
}

// -------------------------------------------------------------------------
// Error clause: open() on a nonexistent path → errno ENOENT (2)
// -------------------------------------------------------------------------

#[test]
fn error_clause_open_nonexistent_extracts_errno() {
    // open("/nonexistent/path", O_RDONLY=0) returns -1, errno = ENOENT (2).
    let func = FfiOpFunction::new(
        "libc.so.6",
        "open",
        vec!["cstr".to_string(), "i32".to_string()],
        "i32",
        true, // error_on_lt_zero
    );
    let result = call(&func, &[json!("/nonexistent/handl-ffi-test-path"), json!(0)]);
    match result {
        Err(EvalError::Other(msg)) => {
            assert!(
                msg.contains("errno: 2"),
                "expected errno 2 (ENOENT) in error message, got: {msg}"
            );
        }
        other => panic!("expected Err(EvalError::Other) with errno, got {other:?}"),
    }
}

#[test]
fn error_clause_disabled_returns_raw_negative() {
    // Same call but with error_on_lt_zero = false — should return -1 as i32.
    let func = FfiOpFunction::new(
        "libc.so.6",
        "open",
        vec!["cstr".to_string(), "i32".to_string()],
        "i32",
        false,
    );
    let result = call(&func, &[json!("/nonexistent/handl-ffi-test-path"), json!(0)])
        .expect("open should succeed (return -1)");
    assert_eq!(result.as_i64(), Some(-1));
}

// -------------------------------------------------------------------------
// Marshal unit tests (no C call needed)
// -------------------------------------------------------------------------

#[test]
fn marshal_i32_value() {
    let mut keep_alive = Vec::new();
    let arg = marshal_arg("i32", &json!(42), &mut keep_alive).expect("marshal i32");
    assert!(matches!(arg, CArg::Int(42)));
    assert!(keep_alive.is_empty());
}

#[test]
fn marshal_cstr_owns_cstring() {
    let mut keep_alive = Vec::new();
    let arg = marshal_arg("cstr", &json!("hi"), &mut keep_alive).expect("marshal cstr");
    match arg {
        CArg::Int(ptr) => {
            assert!(ptr != 0);
        }
        other => panic!("expected CArg::Int for cstr, got {other:?}"),
    }
    assert_eq!(keep_alive.len(), 1);
}

#[test]
fn marshal_unknown_descriptor_errors() {
    let mut keep_alive = Vec::new();
    let result = marshal_arg("bytes", &json!(42), &mut keep_alive);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}
