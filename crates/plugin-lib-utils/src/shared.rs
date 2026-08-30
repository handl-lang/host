use serde_json::{Map, Value as JsonValue};

use crate::function_value::FunctionValue;
use handl_platform::{EvalContext, EvalError};


// ════════════════════════════════════════════════════════════════════
// Tier 0: Shared helpers (replaces duplicated copies across modules)
// ════════════════════════════════════════════════════════════════════

// ── type requirements ──────────────────────────────────────────────

pub(crate) fn require_array(arg: &serde_json::Value) -> Result<&[serde_json::Value], EvalError> {
    arg.as_array().map(Vec::as_slice).ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "array", "non-array",)))
}

pub(crate) fn require_function_value(arg: &serde_json::Value) -> Result<FunctionValue, EvalError> {
    FunctionValue::from_json(arg).ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "function value", "non-function",)))
}

// ── argument count requirements ────────────────────────────────────

pub(crate) fn require_one_arg(args: &[serde_json::Value]) -> Result<&serde_json::Value, EvalError> {
    args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))
}

#[allow(dead_code)]
pub(crate) fn require_one_array(args: &[serde_json::Value]) -> Result<&[serde_json::Value], EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
    require_array(arg)
}

#[allow(dead_code)]
pub(crate) fn require_two_arrays(
    args: &[serde_json::Value],
) -> Result<(&[serde_json::Value], &[serde_json::Value]), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
    }
    let a = require_array(&args[0])?;
    let b = require_array(&args[1])?;
    Ok((a, b))
}

pub(crate) fn require_function_and_array(
    args: &[serde_json::Value],
) -> Result<(FunctionValue, &[serde_json::Value]), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
    }
    let fn_val = require_function_value(&args[0])?;
    let array = require_array(&args[1])?;
    Ok((fn_val, array))
}

pub(crate) fn require_functions(args: &[serde_json::Value]) -> Result<Vec<FunctionValue>, EvalError> {
    if args.is_empty() {
        return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)));
    }
    args.iter().map(require_function_value).collect()
}

// ── param binding ──────────────────────────────────────────────────

pub(crate) fn bind_param(
    fn_val: &FunctionValue,
    value: &serde_json::Value,
) -> Vec<(String, serde_json::Value)> {
    fn_val
        .params
        .first()
        .map(|(name, _)| vec![(name.clone(), serde_json::Value::from(value.clone()))])
        .unwrap_or_default()
}

pub(crate) fn bind_two_params(
    fn_val: &FunctionValue,
    acc: &serde_json::Value,
    item: &serde_json::Value,
) -> Vec<(String, serde_json::Value)> {
    let param_names: Vec<&String> = fn_val.params.iter().map(|(name, _)| name).collect();
    let acc_name = param_names.first().map(|s| s.as_str()).unwrap_or("acc");
    let item_name = param_names.get(1).map(|s| s.as_str()).unwrap_or("x");
    vec![
        (acc_name.to_string(), serde_json::Value::from(acc.clone())),
        (item_name.to_string(), serde_json::Value::from(item.clone())),
    ]
}

fn pattern_binding_name(pattern: &JsonValue) -> Option<&str> {
    pattern.get("type").and_then(|v| v.as_str())?;
    pattern.get("name").and_then(|v| v.as_str())
}

fn destructure_pattern(
    pattern: &JsonValue,
    value: &JsonValue,
    out: &mut Vec<(String, JsonValue)>,
) -> Result<(), EvalError> {
    match pattern.get("type").and_then(|v| v.as_str()) {
        Some("identifier") => {
            if let Some(name) = pattern_binding_name(pattern) {
                out.push((name.to_string(), value.clone()));
            }
        }
        Some("array") => {
            let patterns = pattern
                .get("patterns")
                .and_then(|v| v.as_array())
                .ok_or(EvalError::Other("invalid array pattern".to_string()))?;
            let arr = value.as_array().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "array", "non-array",)))?;
            for (i, child_pattern) in patterns.iter().enumerate() {
                let child = arr.get(i).unwrap_or(&JsonValue::Null);
                destructure_pattern(child_pattern, child, out)?;
            }
        }
        Some("object") => {
            let obj = value.as_object().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "object", "non-object",)))?;
            let entries = pattern
                .get("entries")
                .and_then(|v| v.as_array())
                .ok_or(EvalError::Other("invalid object pattern".to_string()))?;
            for entry in entries {
                let arr = entry.as_array().ok_or(EvalError::Other(
                    "invalid object pattern entry".to_string(),
                ))?;
                let key = arr
                    .get(0)
                    .and_then(|v| v.as_str())
                    .ok_or(EvalError::Other(
                        "invalid object pattern key".to_string(),
                    ))?;
                let child_pattern = arr.get(1).ok_or(EvalError::Other(
                    "invalid object pattern entry".to_string(),
                ))?;
                let child = obj.get(key).unwrap_or(&JsonValue::Null);
                destructure_pattern(child_pattern, child, out)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn bind_function_params(
    fn_val: &FunctionValue,
    args: &[JsonValue],
) -> Result<Vec<(String, serde_json::Value)>, EvalError> {
    let mut bindings = Vec::new();

    for (i, (name, _)) in fn_val.params.iter().enumerate() {
        let arg = args.get(i).unwrap_or(&JsonValue::Null);

        if let Some(pattern) = fn_val.patterns.get(i) {
            if pattern.get("type").is_some() {
                let mut raw = Vec::new();
                destructure_pattern(pattern, arg, &mut raw)?;
                bindings.extend(raw.into_iter().map(|(n, v)| (n, serde_json::Value::from(v))));
                continue;
            }
        }

        bindings.push((name.clone(), serde_json::Value::from(arg.clone())));
    }

    Ok(bindings)
}

// ── truthiness ─────────────────────────────────────────────────────

pub(crate) fn is_truthy_value(value: &serde_json::Value) -> bool {
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

// ── constructors ───────────────────────────────────────────────────

pub(crate) fn arr(items: &[serde_json::Value]) -> serde_json::Value {
    serde_json::Value::from(serde_json::Value::Array(items.to_vec()))
}

pub(crate) fn b(value: bool) -> serde_json::Value {
    serde_json::Value::from(value)
}

pub(crate) fn num(value: f64) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(value))
}

// ── string conversion ──────────────────────────────────────────────

pub(crate) fn key_to_string(value: &serde_json::Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else if let Some(n) = value.as_f64() {
        if n == n.trunc() {
            format!("{n:.0}")
        } else {
            n.to_string()
        }
    } else if let Some(b) = value.as_bool() {
        b.to_string()
    } else if value.is_null() {
        "null".to_string()
    } else {
        serde_json::to_string(&value.clone()).unwrap_or_else(|_| "unknown".to_string())
    }
}

#[allow(dead_code)]
pub(crate) fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f == f.trunc() {
                    format!("{f:.0}")
                } else {
                    f.to_string()
                }
            } else {
                n.to_string()
            }
        }
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "unknown".to_string()),
    }
}

// ── extremum selection ─────────────────────────────────────────────

/// Returns a reference to the element selected by `better` over all items.
/// `better(candidate, best)` should be true when `candidate` should replace `best`.
pub(crate) fn find_extremum_by<'a, T>(
    items: &'a [T],
    mut better: impl FnMut(&T, &T) -> bool,
) -> Option<&'a T> {
    items
        .iter()
        .reduce(|best, x| if better(x, best) { x } else { best })
}

// ── type predicates (mood / type.*) ─────────────────────────────────

pub(crate) fn require_arg_count(args: &[serde_json::Value], expected: usize) -> Result<(), EvalError> {
    if args.len() != expected {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            expected,
            args.len()
        )));
    }
    Ok(())
}

pub(crate) fn as_kind(value: &serde_json::Value) -> Result<&str, EvalError> {
    value
        .as_str()
        .ok_or_else(|| EvalError::Other("type kind must be a string".to_string()))
}

pub(crate) fn type_name(value: &serde_json::Value) -> &'static str {
    if value.is_null() {
        "null"
    } else if value.is_boolean() {
        "bool"
    } else if value.is_number() {
        "num"
    } else if value.is_string() {
        "str"
    } else if value.is_array() {
        "arr"
    } else if value.is_object() {
        "obj"
    } else {
        "unknown"
    }
}

pub(crate) fn matches(value: &serde_json::Value, kind: &str) -> bool {
    if let Some(rest) = kind.strip_prefix("type.") {
        match rest {
            "any" | "unknown" => true,
            "never" => false,
            "null" => value.is_null(),
            "bool" | "boolean" | "bin" => value.is_boolean(),
            "num" | "number" => value.is_number(),
            "str" | "string" => value.is_string(),
            "arr" | "array" => value.is_array(),
            "obj" | "object" => value.is_object(),
            _ => false,
        }
    } else if let Some(rest) = kind.strip_prefix("field.") {
        value.as_object().is_some_and(|o| o.get(rest).is_some())
    } else {
        value.as_object().is_some_and(|o| o.get(kind).is_some())
    }
}

pub(crate) fn project(value: &serde_json::Value, kind: &str) -> Option<serde_json::Value> {
    if kind.starts_with("type.") {
        if matches(value, kind) {
            Some(serde_json::Value::from(value.clone()))
        } else {
            None
        }
    } else if let Some(rest) = kind.strip_prefix("field.") {
        value
            .as_object()
            .and_then(|o| o.get(rest))
            .map(|v| serde_json::Value::from(v.clone()))
    } else {
        value
            .as_object()
            .and_then(|o| o.get(kind))
            .map(|v| serde_json::Value::from(v.clone()))
    }
}

pub(crate) fn complement(value: &serde_json::Value, kind: &str) -> Option<serde_json::Value> {
    if matches(value, kind) {
        return None;
    }

    if kind.starts_with("type.") || kind.starts_with("field.") {
        return Some(serde_json::Value::from(value.clone()));
    }

    // Variant tag: the complement is another key in the same object,
    // or the value itself if it is not a tagged object.
    if let Some(obj) = value.as_object() {
        obj.iter()
            .find(|(k, _)| k.as_str() != kind)
            .map(|(_, v)| serde_json::Value::from(v.clone()))
    } else {
        Some(serde_json::Value::from(value.clone()))
    }
}

pub(crate) fn ok_result(value: serde_json::Value) -> serde_json::Value {
    serde_json::Value::from(JsonValue::Object(Map::from_iter([(
        "ok".to_string(),
        value,
    )])))
}

pub(crate) fn err_result(message: &str) -> serde_json::Value {
    serde_json::Value::from(JsonValue::Object(Map::from_iter([(
        "err".to_string(),
        JsonValue::String(message.to_string()),
    )])))
}

// ════════════════════════════════════════════════════════════════════
// Tier 1: Composable primitives
// ════════════════════════════════════════════════════════════════════

/// Apply a single-param function to a value: bind_param + eval_expr.
pub(crate) fn apply_fn(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    value: &serde_json::Value,
) -> Result<serde_json::Value, EvalError> {
    let vars = bind_param(fn_val, value);
    ctx.eval_expr(&fn_val.body, &vars)
}

/// Apply a two-param function to two values: bind_two_params + eval_expr.
pub(crate) fn apply_fn2(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    a: &serde_json::Value,
    b: &serde_json::Value,
) -> Result<serde_json::Value, EvalError> {
    let vars = bind_two_params(fn_val, a, b);
    ctx.eval_expr(&fn_val.body, &vars)
}

/// Apply a predicate function and check if the result is truthy.
pub(crate) fn is_truthy_fn(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    value: &serde_json::Value,
) -> Result<bool, EvalError> {
    let result = apply_fn(ctx, fn_val, value)?;
    Ok(is_truthy_value(&result))
}

/// Map a function over an iterator, collecting results.
/// Works with any iterator yielding `&serde_json::Value`.
pub(crate) fn map_iter<'a, I>(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    iter: I,
) -> Result<Vec<serde_json::Value>, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    let mut results = Vec::new();
    for item in iter {
        let mapped = apply_fn(ctx, fn_val, item)?;
        results.push(mapped);
    }
    Ok(results)
}

/// Filter an iterator by a predicate function, collecting matching items.
pub(crate) fn filter_iter<'a, I>(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    iter: I,
) -> Result<Vec<serde_json::Value>, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    let mut results = Vec::new();
    for item in iter {
        if is_truthy_fn(ctx, fn_val, item)? {
            results.push(item.clone());
        }
    }
    Ok(results)
}

/// Fold an iterator with a two-param function and an initial accumulator.
pub(crate) fn fold_iter<'a, I>(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    iter: I,
    init: serde_json::Value,
) -> Result<serde_json::Value, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    let mut acc = init;
    for item in iter {
        let result = apply_fn2(ctx, fn_val, &acc, item)?;
        acc = result;
    }
    Ok(acc)
}

/// Find the first element matching a predicate.
/// Returns `None` if no match.
pub(crate) fn find_first<'a, I>(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    iter: I,
) -> Result<Option<serde_json::Value>, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    for item in iter {
        if is_truthy_fn(ctx, fn_val, item)? {
            return Ok(Some(item.clone()));
        }
    }
    Ok(None)
}

/// Find the index of the first element matching a predicate.
/// Returns `None` if no match.
#[allow(dead_code)]
pub(crate) fn find_index<'a, I>(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    iter: I,
) -> Result<Option<usize>, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    for (i, item) in iter.enumerate() {
        if is_truthy_fn(ctx, fn_val, item)? {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

/// Find the first matching element from a reverse iterator.
/// Returns `None` if no match.
#[allow(dead_code)]
pub(crate) fn find_first_rev<'a, I>(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    iter: I,
) -> Result<Option<serde_json::Value>, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    find_first(ctx, fn_val, iter)
}

/// Find the index of the first matching element from a reverse iterator.
/// The index is relative to the reversed iteration order.
/// Returns `None` if no match.
#[allow(dead_code)]
pub(crate) fn find_index_rev<'a, I>(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    iter: I,
) -> Result<Option<usize>, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    find_index(ctx, fn_val, iter)
}
