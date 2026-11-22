use handl_platform::{EvalContext, EvalError};

use crate::function_value::FunctionValue;

/// `{"__arrow__": true, …}` wire value — the callable shape produced by
/// the tree-walk/host eval (ADR-0106).
pub(super) fn is_arrow_value(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|obj| obj.get("__arrow__"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Call a callable value with positional arguments.
///
/// Arrow-values (`{"__arrow__": true, …}`) are dispatched through
/// [`EvalContext::try_call_value`] — the only call path that works under
/// the tree-walk eval, where `eval_expr` has no wired evaluator. Legacy
/// `FunctionValue` JSON keeps the serialized-body + `eval_expr` path
/// (positional parameter binding, missing args bind `Null`).
pub(super) fn call_callable(
    ctx: &mut EvalContext,
    callee: &serde_json::Value,
    args: &[serde_json::Value],
) -> Result<serde_json::Value, EvalError> {
    if is_arrow_value(callee) {
        return ctx.try_call_value(callee, args).ok_or_else(|| {
            EvalError::Other("call_value: no value callable set in context".to_string())
        })?;
    }
    let fn_val = FunctionValue::from_json(callee).ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "function value", "non-function",
    )))?;
    let vars: Vec<(String, serde_json::Value)> = fn_val
        .params
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            (
                name.clone(),
                args.get(i).cloned().unwrap_or(serde_json::Value::Null),
            )
        })
        .collect();
    ctx.eval_expr(&fn_val.body, &vars)
}

/// Single-argument convenience wrapper over [`call_callable`].
pub(super) fn call_callable1(
    ctx: &mut EvalContext,
    callee: &serde_json::Value,
    value: &serde_json::Value,
) -> Result<serde_json::Value, EvalError> {
    call_callable(ctx, callee, std::slice::from_ref(value))
}

/// Handl-style truthiness for callable results.
pub(super) fn is_truthy_value(value: &serde_json::Value) -> bool {
    if let Some(b) = value.as_bool() {
        b
    } else if let Some(n) = value.as_f64() {
        n != 0.0
    } else if let Some(s) = value.as_str() {
        !s.is_empty()
    } else if value.is_null() {
        false
    } else if let Some(arr) = value.as_array() {
        !arr.is_empty()
    } else if let Some(obj) = value.as_object() {
        !obj.is_empty()
    } else {
        true
    }
}

/// Call a predicate callable and report whether its result is truthy.
pub(super) fn is_truthy_callable(
    ctx: &mut EvalContext,
    callee: &serde_json::Value,
    value: &serde_json::Value,
) -> Result<bool, EvalError> {
    Ok(is_truthy_value(&call_callable1(ctx, callee, value)?))
}

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
