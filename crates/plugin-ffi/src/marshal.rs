//! Marshalling between `serde_json::Value` and C ABI types.
//!
//! The dispatcher marshals each handl argument to a C type per a descriptor
//! string, calls the C symbol, and then unmarshals the return value back to a
//! JSON value.
//!
//! ## Marshalling descriptors
//!
//! | Descriptor     | C type                  | JSON source          |
//! |----------------|-------------------------|----------------------|
//! | `"i32"`        | `int32_t` (value)       | number (range-checked) |
//! | `"i64"`        | `int64_t` (value)       | number               |
//! | `"f64"`        | `double` (value)        | number               |
//! | `"cstr"`       | `const char *`          | string (NUL-checked) |
//! | `"ptr"`        | `void *`                | number (address)     |
//! | `"handle"`     | `void *`                | `{"__resource__": {"ptr": N}}` (opaque handle, ADR-0083 §2.2) |
//! | `"void"`       | `void` (return)         | — (returns null)     |
//! | `"in_buf"`     | `const void * + size_t` | `Bytes`              |
//! | `"cbytes"`     | `const void * + size_t` | `Bytes`              |
//! | `"out_buf:N"`  | `void * + size_t`       | — (allocates N)      |
//!
//! `in_buf` / `cbytes` / `out_buf:N` are the buffer descriptors introduced
//! with the `Bytes` type (ADR-0078 Phase 2 / ADR-0083 §2.1). `in_buf` and
//! `cbytes` each expand to **two** C arguments — `(ptr, len)` — and consume
//! one handl `Bytes` argument. `out_buf:N` allocates `N` bytes, passes
//! `(ptr, N)` as two C arguments, consumes **no** handl argument, and turns
//! the return into a record `{ ret, buf }` where `buf` is the first `ret`
//! bytes of the buffer (clamped to `N`; empty when `ret < 0`).
//!
//! `Bytes` values are carried as `serde_json::Value` using the
//! `{"__bytes__": [n, ...]}` encoding (matching `TaggedValue::Bytes` in
//! `handl-value`). For input, a plain JSON array of numbers is also accepted
//! as a convenience.

use std::ffi::{CStr, CString, c_char};

use handl_platform::EvalError;
use serde_json::{Map, Value as JsonValue};

// ---------------------------------------------------------------------------
// Intermediate C-side representations
// ---------------------------------------------------------------------------

/// A marshalled C argument ready to be passed across the FFI boundary.
///
/// All integer and pointer descriptors (`i32`, `i64`, `ptr`, `cstr`) are
/// widened to `i64`. This is sound on x86-64 SysV and AArch64 where a 32-bit
/// integer argument is sign/zero-extended in its calling register, so passing
/// the full 64-bit slot is equivalent. Floating-point values stay `f64`
/// because they travel in separate (XMM) registers and must not be
/// bit-cast to an integer slot.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CArg {
    /// An integer or pointer argument, widened to 64 bits.
    Int(i64),
    /// A double-precision floating-point argument.
    Float(f64),
}

/// The *kind* of return value expected from the C function.
///
/// This drives which function-pointer signature the dispatcher transmutes the
/// raw symbol to. Crucially, `Int32` and `Int64` produce *different* return
/// types in the transmuted signature: on x86-64 a function returning `int`
/// only writes the lower 32 bits of `rax` (the upper 32 are zeroed, **not**
/// sign-extended), so reading the full 64-bit `rax` as `i64` would turn a
/// `-1` `int` return into `4_294_967_295`. Reading it as `i32` and then
/// sign-extending to `i64` gives the correct `-1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CRetKind {
    /// 32-bit integer return (read as `i32`, sign-extended to `i64`).
    Int32,
    /// 64-bit integer or pointer return (read as `i64`).
    Int64,
    /// Double-precision floating-point return.
    Float,
    /// No return value.
    Void,
}

/// A value returned from a C function.
pub(crate) enum CRet {
    /// Integer or pointer return.
    Int(i64),
    /// Floating-point return.
    Float(f64),
    /// Void return.
    Void,
}

// ---------------------------------------------------------------------------
// Marshalling: JSON -> C
// ---------------------------------------------------------------------------

/// Marshal a single JSON argument to a [`CArg`] per its descriptor string.
///
/// For `cstr` descriptors the produced [`CString`] is pushed into
/// `keep_alive` so it lives until the C call returns; the returned [`CArg`]
/// only holds the raw pointer value.
pub(crate) fn marshal_arg(
    descriptor: &str,
    value: &JsonValue,
    keep_alive: &mut Vec<CString>,
) -> Result<CArg, EvalError> {
    match descriptor {
        "i32" => {
            let n = value
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number for i32".to_string()))?;
            let v = i32::try_from(n)
                .map_err(|_| EvalError::TypeMismatch(format!("i32 overflow: {n}")))?;
            Ok(CArg::Int(v as i64))
        }
        "i64" => {
            let n = value
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number for i64".to_string()))?;
            Ok(CArg::Int(n))
        }
        "f64" => {
            let n = value
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number for f64".to_string()))?;
            Ok(CArg::Float(n))
        }
        "cstr" => {
            let s = value
                .as_str()
                .ok_or_else(|| EvalError::TypeMismatch("expected string for cstr".to_string()))?;
            let c = CString::new(s)
                .map_err(|_| EvalError::TypeMismatch("embedded NUL byte in cstr".to_string()))?;
            let ptr = c.as_ptr();
            keep_alive.push(c);
            Ok(CArg::Int(ptr as i64))
        }
        "ptr" => {
            let n = value
                .as_i64()
                .ok_or_else(|| EvalError::TypeMismatch("expected number for ptr".to_string()))?;
            Ok(CArg::Int(n))
        }
        // `handle` accepts a `{"__resource__": {"ptr": N}}` JSON value (the
        // debug form of `TaggedValue::Resource`) and passes the raw pointer
        // through to the C call. A bare number is also accepted for
        // convenience (e.g. a POSIX fd passed as an i32).
        "handle" => {
            if let Some(ptr_n) = value
                .as_object()
                .and_then(|obj| obj.get("__resource__"))
                .and_then(|r| r.get("ptr"))
                .and_then(|v| v.as_i64())
            {
                return Ok(CArg::Int(ptr_n));
            }
            let n = value.as_i64().ok_or_else(|| {
                EvalError::TypeMismatch("expected resource or number for handle".to_string())
            })?;
            Ok(CArg::Int(n))
        }
        other => Err(EvalError::TypeMismatch(format!(
            "unknown arg descriptor: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Buffer descriptors: in_buf / cbytes / out_buf:N (ADR-0083 §2.1)
// ---------------------------------------------------------------------------

/// An allocated output buffer for an `out_buf:N` descriptor.
///
/// Owns the backing `Vec<u8>` so it stays valid across the C call and is
/// available for the post-call `{ ret, buf }` extraction.
pub(crate) struct OutBuf {
    /// The allocated buffer (zero-initialized). Valid until this is dropped.
    pub buf: Vec<u8>,
    /// The declared capacity `N` (== `buf.len()`).
    pub len: usize,
}

/// The result of marshalling a whole argument list.
///
/// `c_args` is the flat list of C arguments to pass (buffer descriptors
/// expand to two entries each). `keep_alive` / `byte_keep_alive` keep the
/// backing storage alive for the duration of the C call. `out_bufs` records
/// any `out_buf:N` slots so the return can be wrapped as `{ ret, buf }`.
pub(crate) struct MarshalResult {
    pub c_args: Vec<CArg>,
    /// `CString`s owned by the marshaller (for `cstr` args).
    pub keep_alive: Vec<CString>,
    /// Source `Vec<u8>`s for `in_buf` / `cbytes` args (C reads from these).
    pub byte_keep_alive: Vec<Vec<u8>>,
    /// Allocated output buffers for `out_buf:N` descriptors.
    pub out_bufs: Vec<OutBuf>,
}

/// Decode a JSON `Bytes` value into a `Vec<u8>`.
///
/// Accepts either the canonical `{"__bytes__": [n, ...]}` encoding (matching
/// `TaggedValue::Bytes`) or, as a convenience, a plain JSON array of numbers.
/// Each element must be an integer in `0..=255`; out-of-range or non-integer
/// elements are a marshal error rather than a silent truncation.
pub(crate) fn json_to_bytes(value: &JsonValue) -> Result<Vec<u8>, EvalError> {
    let arr: &[JsonValue] = if let Some(obj) = value.as_object() {
        match obj.get("__bytes__").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => {
                return Err(EvalError::TypeMismatch(
                    "expected Bytes ({__bytes__: [numbers]})".to_string(),
                ));
            }
        }
    } else if let Some(a) = value.as_array() {
        a
    } else {
        return Err(EvalError::TypeMismatch(
            "expected Bytes value (object or array)".to_string(),
        ));
    };
    let mut out = Vec::with_capacity(arr.len());
    for el in arr {
        let n = el.as_i64().ok_or_else(|| {
            EvalError::TypeMismatch("Bytes array must contain numbers".to_string())
        })?;
        let b = u8::try_from(n)
            .map_err(|_| EvalError::TypeMismatch(format!("byte out of range 0..=255: {n}")))?;
        out.push(b);
    }
    Ok(out)
}

/// Encode a byte slice as the canonical `{"__bytes__": [n, ...]}` JSON value.
pub(crate) fn bytes_to_json(bytes: &[u8]) -> JsonValue {
    let arr: Vec<JsonValue> = bytes.iter().map(|&b| JsonValue::from(b as i64)).collect();
    let mut map = Map::new();
    map.insert("__bytes__".to_string(), JsonValue::Array(arr));
    JsonValue::Object(map)
}

/// Marshal a full argument list, expanding buffer descriptors.
///
/// Unlike [`marshal_arg`] (which handles a single C argument), this walks the
/// whole descriptor list because `in_buf` / `cbytes` / `out_buf:N` each
/// expand to **two** C arguments. `out_buf:N` consumes no handl argument; the
/// other descriptors consume exactly one. The number of handl arguments must
/// equal the number of descriptors that consume one.
pub(crate) fn marshal_call_args(
    descriptors: &[String],
    args: &[JsonValue],
) -> Result<MarshalResult, EvalError> {
    let mut res = MarshalResult {
        c_args: Vec::with_capacity(descriptors.len() * 2),
        keep_alive: Vec::with_capacity(descriptors.len()),
        byte_keep_alive: Vec::with_capacity(descriptors.len()),
        out_bufs: Vec::new(),
    };
    let mut arg_idx = 0usize;
    for desc in descriptors {
        if desc == "in_buf" || desc == "cbytes" {
            let value = args
                .get(arg_idx)
                .ok_or_else(|| EvalError::TypeMismatch(format!("missing argument for {desc}")))?;
            arg_idx += 1;
            let bytes = json_to_bytes(value)?;
            let ptr = bytes.as_ptr() as i64;
            let len = bytes.len() as i64;
            // Move `bytes` into keep-alive *after* capturing the pointer; the
            // Vec's heap allocation is stable across the move.
            res.byte_keep_alive.push(bytes);
            res.c_args.push(CArg::Int(ptr));
            res.c_args.push(CArg::Int(len));
        } else if let Some(n_str) = desc.strip_prefix("out_buf:") {
            let n: usize = n_str
                .parse()
                .map_err(|_| EvalError::TypeMismatch(format!("invalid out_buf size: {n_str:?}")))?;
            let mut buf = vec![0u8; n];
            let ptr = buf.as_mut_ptr() as i64;
            let len = buf.len() as i64;
            res.c_args.push(CArg::Int(ptr));
            res.c_args.push(CArg::Int(len));
            res.out_bufs.push(OutBuf { buf, len: n });
        } else {
            let value = args
                .get(arg_idx)
                .ok_or_else(|| EvalError::TypeMismatch(format!("missing argument for {desc}")))?;
            arg_idx += 1;
            let arg = marshal_arg(desc, value, &mut res.keep_alive)?;
            res.c_args.push(arg);
        }
    }
    if arg_idx != args.len() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {arg_idx} handl args, got {}",
            args.len()
        )));
    }
    Ok(res)
}

/// Wrap a raw C return as a `{ ret, buf }` record when `out_buf` was used.
///
/// `raw_ret` is the already-unmarshalled C return value. `buf` is the first
/// `ret` bytes of the output buffer when `ret` is a non-negative count
/// (clamped to the buffer length), or empty when `ret < 0` (error). When
/// there are multiple `out_buf` slots, the first one supplies `buf`.
pub(crate) fn wrap_out_buf_return(raw_ret: &JsonValue, out_bufs: &[OutBuf]) -> JsonValue {
    let ob = match out_bufs.first() {
        Some(ob) => ob,
        None => return raw_ret.clone(),
    };
    let ret_i64 = raw_ret.as_i64().unwrap_or(0);
    let take = if ret_i64 < 0 {
        0
    } else {
        (ret_i64 as usize).min(ob.len)
    };
    let buf_json = bytes_to_json(&ob.buf[..take]);
    let mut map = Map::new();
    map.insert("ret".to_string(), raw_ret.clone());
    map.insert("buf".to_string(), buf_json);
    JsonValue::Object(map)
}

// ---------------------------------------------------------------------------
// Unmarshalling: C -> JSON
// ---------------------------------------------------------------------------

/// Determine the return kind from a return descriptor string.
pub(crate) fn return_kind(descriptor: &str) -> Result<CRetKind, EvalError> {
    match descriptor {
        "i32" => Ok(CRetKind::Int32),
        "i64" | "ptr" | "cstr" | "handle" => Ok(CRetKind::Int64),
        "f64" => Ok(CRetKind::Float),
        "void" => Ok(CRetKind::Void),
        other => Err(EvalError::TypeMismatch(format!(
            "unknown return descriptor: {other}"
        ))),
    }
}

/// Unmarshal a C return value to a JSON value per the return descriptor.
pub(crate) fn unmarshal_return(descriptor: &str, ret: &CRet) -> Result<JsonValue, EvalError> {
    match descriptor {
        "i32" => match ret {
            CRet::Int(v) => Ok(JsonValue::from(*v as i32 as i64)),
            _ => Err(EvalError::Other(
                "expected integer return for i32".to_string(),
            )),
        },
        "i64" => match ret {
            CRet::Int(v) => Ok(JsonValue::from(*v)),
            _ => Err(EvalError::Other(
                "expected integer return for i64".to_string(),
            )),
        },
        "f64" => match ret {
            CRet::Float(v) => serde_json::Number::from_f64(*v)
                .map(JsonValue::Number)
                .ok_or_else(|| EvalError::Other("non-finite f64 return".to_string())),
            _ => Err(EvalError::Other(
                "expected float return for f64".to_string(),
            )),
        },
        "ptr" => match ret {
            CRet::Int(v) => Ok(JsonValue::from(*v)),
            _ => Err(EvalError::Other(
                "expected integer return for ptr".to_string(),
            )),
        },
        // `handle` wraps the raw C return pointer in the JSON debug form of
        // a `TaggedValue::Resource` (ADR-0083 §2.2): `{"__resource__":
        // {"ptr": N}}`. The finalizer cannot be carried across the JSON
        // boundary — when this JSON is decoded back to a `TaggedValue`, the
        // resulting `Resource` has `finalizer = None`. The `FfiOpFunction`
        // stores an optional finalizer field for future use once the
        // dispatcher returns `TaggedValue` directly instead of JSON.
        "handle" => match ret {
            CRet::Int(v) => {
                let mut inner = Map::new();
                inner.insert("ptr".to_string(), JsonValue::from(*v));
                let mut outer = Map::new();
                outer.insert("__resource__".to_string(), JsonValue::Object(inner));
                Ok(JsonValue::Object(outer))
            }
            _ => Err(EvalError::Other(
                "expected integer return for handle".to_string(),
            )),
        },
        "cstr" => match ret {
            CRet::Int(v) => {
                if *v == 0 {
                    return Ok(JsonValue::Null);
                }
                let ptr = *v as *const c_char;
                let s = unsafe { CStr::from_ptr(ptr) }
                    .to_str()
                    .map_err(|e| EvalError::Other(format!("non-utf8 cstr return: {e}")))?;
                Ok(JsonValue::from(s))
            }
            _ => Err(EvalError::Other(
                "expected integer return for cstr".to_string(),
            )),
        },
        "void" => Ok(JsonValue::Null),
        other => Err(EvalError::TypeMismatch(format!(
            "unknown return descriptor: {other}"
        ))),
    }
}
