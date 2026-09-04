//! Marshalling between `serde_json::Value` and C ABI types.
//!
//! The dispatcher marshals each handl argument to a C type per a descriptor
//! string, calls the C symbol, and then unmarshals the return value back to a
//! JSON value. Only simple scalar/pointer types are supported in this phase
//! (ADR-0083 §4); buffer and struct descriptors require a `Bytes` type that
//! does not exist yet.

use std::ffi::{CStr, CString, c_char};

use handl_platform::EvalError;
use serde_json::Value as JsonValue;

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
#[derive(Clone, Copy)]
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
            let c = CString::new(s).map_err(|_| {
                EvalError::TypeMismatch("embedded NUL byte in cstr".to_string())
            })?;
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
        other => Err(EvalError::TypeMismatch(format!(
            "unknown arg descriptor: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Unmarshalling: C -> JSON
// ---------------------------------------------------------------------------

/// Determine the return kind from a return descriptor string.
pub(crate) fn return_kind(descriptor: &str) -> Result<CRetKind, EvalError> {
    match descriptor {
        "i32" => Ok(CRetKind::Int32),
        "i64" | "ptr" | "cstr" => Ok(CRetKind::Int64),
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
