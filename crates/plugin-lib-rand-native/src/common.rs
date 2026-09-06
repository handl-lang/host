/// Require a number argument at `idx`.
use handl_platform::EvalError;
pub(super) fn require_number(args: &[serde_json::Value], idx: usize) -> Result<f64, EvalError> {
    let arg = args.get(idx).ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        idx + 1,
        args.len(),
    )))?;
    arg.as_f64().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "number", "non-number",
    )))
}

/// Require an integer argument at `idx`.
pub(super) fn require_int(args: &[serde_json::Value], idx: usize) -> Result<i64, EvalError> {
    let n = require_number(args, idx)?;
    if !n.is_finite() || n.fract() != 0.0 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "integer", "non-integer number",
        )));
    }
    Ok(n as i64)
}

/// Require an array argument at `idx`.
pub(super) fn require_array(
    args: &[serde_json::Value],
    idx: usize,
) -> Result<&[serde_json::Value], EvalError> {
    let arg = args.get(idx).ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        idx + 1,
        args.len(),
    )))?;
    arg.as_array()
        .map(Vec::as_slice)
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "array", "non-array",
        )))
}

pub(super) fn num(value: impl Into<f64>) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(value.into()))
}

pub(super) fn bool_val(value: bool) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(value))
}

pub(super) fn s(value: impl Into<String>) -> serde_json::Value {
    serde_json::Value::from(value.into())
}

pub(super) fn array(values: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::Value::from(serde_json::Value::Array(values))
}
