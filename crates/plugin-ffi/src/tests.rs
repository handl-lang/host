//! Tests for the FFI dispatcher using real Linux syscalls.
//!
//! All tests are `#[cfg(target_os = "linux")]` because they call into libc
//! directly via `dlopen` / `dlsym` and rely on glibc-specific symbols
//! (`getpid`, `abs`, `labs`, `fabs`, `strlen`, `open`) and errno semantics
//! (`__errno_location`, `ENOENT = 2`).

#![cfg(target_os = "linux")]

use handl_platform::{EvalContext, EvalError, Function};
use serde_json::{Value, json};

use crate::marshal::{
    CArg, CRet, bytes_to_json, json_to_bytes, marshal_arg, unmarshal_return,
};
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

// -------------------------------------------------------------------------
// Bytes marshalling helpers (no C call needed)
// -------------------------------------------------------------------------

#[test]
fn json_to_bytes_accepts_tagged_object() {
    let v = json!({"__bytes__": [0xDE, 0xAD, 0xBE, 0xEF]});
    let b = json_to_bytes(&v).expect("tagged bytes decode");
    assert_eq!(b, vec![0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn json_to_bytes_accepts_plain_array() {
    let v = json!([1, 2, 3, 255]);
    let b = json_to_bytes(&v).expect("plain-array bytes decode");
    assert_eq!(b, vec![1, 2, 3, 255]);
}

#[test]
fn json_to_bytes_rejects_out_of_range() {
    let v = json!({"__bytes__": [256]});
    assert!(matches!(json_to_bytes(&v), Err(EvalError::TypeMismatch(_))));
}

#[test]
fn json_to_bytes_rejects_non_object() {
    let v = json!("not bytes");
    assert!(matches!(json_to_bytes(&v), Err(EvalError::TypeMismatch(_))));
}

#[test]
fn bytes_to_json_roundtrips() {
    let original = vec![0u8, 127, 128, 255];
    let encoded = bytes_to_json(&original);
    let decoded = json_to_bytes(&encoded).expect("roundtrip decode");
    assert_eq!(decoded, original);
    // Encoding is the canonical {"__bytes__": [...]} form.
    assert!(encoded.as_object().unwrap().contains_key("__bytes__"));
}

// -------------------------------------------------------------------------
// Buffer FFI ops via real Linux syscalls (read / write)
//
// Uses memfd_create(2) to obtain an anonymous in-memory file with no path,
// then exercises write(2) via `in_buf` and read(2) via `out_buf:N`, verifying
// the bytes round-trip across the FFI boundary.
// -------------------------------------------------------------------------

/// Open an anonymous memfd for buffer-op tests. Returns the raw fd.
fn memfd() -> i32 {
    let memfd = FfiOpFunction::new(
        "libc.so.6",
        "memfd_create",
        vec!["cstr".to_string(), "i32".to_string()],
        "i32",
        true,
    );
    let fd_val = call(&memfd, &[json!("handl-ffi-bytes-test"), json!(0)])
        .expect("memfd_create should succeed");
    let fd = fd_val.as_i64().expect("fd should be an integer") as i32;
    assert!(fd >= 0, "memfd_create returned invalid fd: {fd}");
    fd
}

/// Close a raw fd, panicking on failure.
fn close_fd(fd: i32) {
    let close_fn = FfiOpFunction::new(
        "libc.so.6",
        "close",
        vec!["i32".to_string()],
        "i32",
        true,
    );
    call(&close_fn, &[json!(fd)]).expect("close should succeed");
}

#[test]
fn in_buf_write_roundtrips_bytes_to_memfd() {
    let fd = memfd();

    // write(fd, buf, count) — `in_buf` expands to (ptr, len).
    let payload = json!({"__bytes__": [0xDE, 0xAD, 0xBE, 0xEF]});
    let write_fn = FfiOpFunction::new(
        "libc.so.6",
        "write",
        vec!["i32".to_string(), "in_buf".to_string()],
        "i64",
        true,
    );
    let written = call(&write_fn, &[json!(fd), payload])
        .expect("write should succeed");
    assert_eq!(written.as_i64(), Some(4));

    close_fd(fd);
}

#[test]
fn out_buf_read_returns_ret_and_buf_record() {
    let fd = memfd();

    // Seed the memfd with 4 known bytes via write(2) + in_buf.
    let payload = json!({"__bytes__": [0xDE, 0xAD, 0xBE, 0xEF]});
    let write_fn = FfiOpFunction::new(
        "libc.so.6",
        "write",
        vec!["i32".to_string(), "in_buf".to_string()],
        "i64",
        true,
    );
    call(&write_fn, &[json!(fd), payload]).expect("write should succeed");

    // Rewind to offset 0: lseek(fd, 0, SEEK_SET=0).
    let lseek = FfiOpFunction::new(
        "libc.so.6",
        "lseek",
        vec!["i32".to_string(), "i64".to_string(), "i32".to_string()],
        "i64",
        true,
    );
    call(&lseek, &[json!(fd), json!(0), json!(0)]).expect("lseek should succeed");

    // read(fd, buf, count) — `out_buf:16` allocates 16 bytes, expands to
    // (ptr, 16), and the return is wrapped as { ret, buf }.
    let read_fn = FfiOpFunction::new(
        "libc.so.6",
        "read",
        vec!["i32".to_string(), "out_buf:16".to_string()],
        "i64",
        true,
    );
    // Only one handl arg (fd): out_buf consumes none.
    let result = call(&read_fn, &[json!(fd)]).expect("read should succeed");
    let obj = result
        .as_object()
        .expect("read result should be a {ret, buf} record");
    assert_eq!(
        obj.get("ret").and_then(|v| v.as_i64()),
        Some(4),
        "ret should be the byte count read"
    );
    let buf = obj.get("buf").expect("buf field should be present");
    let bytes_arr = buf
        .get("__bytes__")
        .and_then(|v| v.as_array())
        .expect("buf should be a {__bytes__:[...]} value");
    let got: Vec<u8> = bytes_arr
        .iter()
        .map(|v| v.as_i64().unwrap() as u8)
        .collect();
    assert_eq!(got, vec![0xDE, 0xAD, 0xBE, 0xEF]);

    close_fd(fd);
}

#[test]
fn out_buf_clamps_buf_to_ret_count() {
    // read into a 16-byte buffer from an empty memfd: ret == 0, buf == [].
    let fd = memfd();
    let read_fn = FfiOpFunction::new(
        "libc.so.6",
        "read",
        vec!["i32".to_string(), "out_buf:16".to_string()],
        "i64",
        true,
    );
    let result = call(&read_fn, &[json!(fd)]).expect("read should succeed");
    let obj = result.as_object().expect("{ret, buf} record");
    assert_eq!(obj.get("ret").and_then(|v| v.as_i64()), Some(0));
    let bytes_arr = obj
        .get("buf")
        .and_then(|v| v.get("__bytes__"))
        .and_then(|v| v.as_array())
        .expect("buf bytes array");
    assert!(bytes_arr.is_empty(), "buf should be empty when ret == 0");

    close_fd(fd);
}

#[test]
fn cbytes_passes_ptr_len_pair_to_write() {
    // `cbytes` is semantically identical to `in_buf` for write(2): it passes
    // (ptr, len) with no NUL termination or UTF-8 validation. Verify a buffer
    // containing a NUL byte round-trips (cstr would reject this).
    let fd = memfd();
    let payload = json!({"__bytes__": [0x00, 0x01, 0x02, 0xFF]});
    let write_fn = FfiOpFunction::new(
        "libc.so.6",
        "write",
        vec!["i32".to_string(), "cbytes".to_string()],
        "i64",
        true,
    );
    let written = call(&write_fn, &[json!(fd), payload]).expect("write should succeed");
    assert_eq!(written.as_i64(), Some(4));

    // Read it back to confirm the NUL byte survived.
    let lseek = FfiOpFunction::new(
        "libc.so.6",
        "lseek",
        vec!["i32".to_string(), "i64".to_string(), "i32".to_string()],
        "i64",
        true,
    );
    call(&lseek, &[json!(fd), json!(0), json!(0)]).expect("lseek should succeed");
    let read_fn = FfiOpFunction::new(
        "libc.so.6",
        "read",
        vec!["i32".to_string(), "out_buf:16".to_string()],
        "i64",
        true,
    );
    let result = call(&read_fn, &[json!(fd)]).expect("read should succeed");
    let bytes_arr = result
        .get("buf")
        .and_then(|v| v.get("__bytes__"))
        .and_then(|v| v.as_array())
        .expect("buf bytes array");
    let got: Vec<u8> = bytes_arr.iter().map(|v| v.as_i64().unwrap() as u8).collect();
    assert_eq!(got, vec![0x00, 0x01, 0x02, 0xFF]);

    close_fd(fd);
}

#[test]
fn out_buf_invalid_size_errors() {
    let read_fn = FfiOpFunction::new(
        "libc.so.6",
        "read",
        vec!["i32".to_string(), "out_buf:notanumber".to_string()],
        "i64",
        true,
    );
    let result = call(&read_fn, &[json!(0)]);
    assert!(
        matches!(result, Err(EvalError::TypeMismatch(_))),
        "expected TypeMismatch for invalid out_buf size, got {result:?}"
    );
}

#[test]
fn in_buf_wrong_value_type_errors() {
    let write_fn = FfiOpFunction::new(
        "libc.so.6",
        "write",
        vec!["i32".to_string(), "in_buf".to_string()],
        "i64",
        true,
    );
    // Passing a string where Bytes is expected.
    let result = call(&write_fn, &[json!(1), json!("not bytes")]);
    assert!(
        matches!(result, Err(EvalError::TypeMismatch(_))),
        "expected TypeMismatch for non-Bytes in_buf, got {result:?}"
    );
}
