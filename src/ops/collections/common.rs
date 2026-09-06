use handl_platform::EvalError;

/// Require an argument at `idx` to be an array.
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

/// Require a single array argument.
pub(super) fn require_one_array(
    args: &[serde_json::Value],
) -> Result<&[serde_json::Value], EvalError> {
    if args.is_empty() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )));
    }
    require_array(args, 0)
}

/// Require an argument at `idx` to be an object.
pub(super) fn require_object(
    args: &[serde_json::Value],
    idx: usize,
) -> Result<&serde_json::Map<String, serde_json::Value>, EvalError> {
    let arg = args.get(idx).ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        idx + 1,
        args.len(),
    )))?;
    arg.as_object().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "object", "non-object",
    )))
}

/// Require two array arguments.
pub(super) fn require_two_arrays(
    args: &[serde_json::Value],
) -> Result<(&[serde_json::Value], &[serde_json::Value]), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    Ok((require_array(args, 0)?, require_array(args, 1)?))
}

/// Require a single object argument.
pub(super) fn require_one_object(
    args: &[serde_json::Value],
) -> Result<&serde_json::Map<String, serde_json::Value>, EvalError> {
    if args.is_empty() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )));
    }
    require_object(args, 0)
}

pub(super) fn num(value: impl Into<f64>) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(value.into()))
}

pub(super) fn b(value: bool) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(value))
}

pub(super) fn arr(values: &[serde_json::Value]) -> serde_json::Value {
    serde_json::Value::from(serde_json::Value::Array(values.to_vec()))
}

pub(super) fn obj(map: serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    serde_json::Value::from(serde_json::Value::Object(map))
}
