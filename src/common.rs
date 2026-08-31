use crate::error::CryptoError;
use handl_platform::EvalError;

/// Extract bytes from an argument that is either a string (UTF-8) or an
/// array of integers in the range 0–255.
pub(super) fn extract_bytes(args: &[serde_json::Value], idx: usize) -> Result<Vec<u8>, EvalError> {
    let arg = args.get(idx).ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        idx + 1,
        args.len(),
    )))?;

    if let Some(s) = arg.as_str() {
        return Ok(s.as_bytes().to_vec());
    }

    if let Some(arr) = arg.as_array() {
        let mut bytes = Vec::with_capacity(arr.len());
        for item in arr.iter() {
            let n = item.as_f64().ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "number", "non-number",
            )))?;
            if !n.is_finite() || n < 0.0 || n > 255.0 {
                return Err(CryptoError::InvalidByteValue { value: n }.into());
            }
            bytes.push(n as u8);
        }
        return Ok(bytes);
    }

    Err(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string or array of numbers", "non-string non-array",
    )))
}

pub(super) fn s(value: impl Into<String>) -> serde_json::Value {
    serde_json::Value::from(value.into())
}
