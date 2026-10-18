use crate::function_value::FunctionValue;
use handl_platform::{EvalContext, EvalError, Function};

use super::common::{arr, num};

// ════════════════════════════════════════════════════════════════════
// Shared helpers for array functions
// ════════════════════════════════════════════════════════════════════

fn require_array_value(arg: &serde_json::Value) -> Result<&[serde_json::Value], EvalError> {
    arg.as_array()
        .map(Vec::as_slice)
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "array", "non-array",
        )))
}

fn require_function_value(arg: &serde_json::Value) -> Result<FunctionValue, EvalError> {
    FunctionValue::from_json(arg).ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "function value", "non-function",
    )))
}

fn is_truthy_value(value: &serde_json::Value) -> bool {
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

fn bind_param(
    fn_val: &FunctionValue,
    value: &serde_json::Value,
) -> Vec<(String, serde_json::Value)> {
    fn_val
        .params
        .first()
        .map(|(name, _)| vec![(name.clone(), serde_json::Value::from(value.clone()))])
        .unwrap_or_default()
}

fn bind_two_params(
    fn_val: &FunctionValue,
    a: &serde_json::Value,
    b: &serde_json::Value,
) -> Vec<(String, serde_json::Value)> {
    let names: Vec<&String> = fn_val.params.iter().map(|(name, _)| name).collect();
    let a_name = names.first().map(|s| s.as_str()).unwrap_or("acc");
    let b_name = names.get(1).map(|s| s.as_str()).unwrap_or("x");
    vec![
        (a_name.to_string(), serde_json::Value::from(a.clone())),
        (b_name.to_string(), serde_json::Value::from(b.clone())),
    ]
}

fn apply_fn(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    value: &serde_json::Value,
) -> Result<serde_json::Value, EvalError> {
    let vars = bind_param(fn_val, value);
    ctx.eval_expr(&fn_val.body, &vars)
}

fn apply_fn2(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    a: &serde_json::Value,
    b: &serde_json::Value,
) -> Result<serde_json::Value, EvalError> {
    let vars = bind_two_params(fn_val, a, b);
    ctx.eval_expr(&fn_val.body, &vars)
}

fn is_truthy_fn(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    value: &serde_json::Value,
) -> Result<bool, EvalError> {
    let result = apply_fn(ctx, fn_val, value)?;
    Ok(is_truthy_value(&result))
}

fn find_first<'a, I>(
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

fn find_index<'a, I>(
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

#[derive(Clone, Copy)]
pub struct Push;
impl Function for Push {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let mut result = array.to_vec();
        for arg in &args[1..] {
            result.push(arg.clone());
        }
        Ok(arr(&result))
    }
}

// ════════════════════════════════════════════════════════════════════
// Iteration operations
// ════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
pub struct TakeWhile;
impl Function for TakeWhile {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let mut result = Vec::new();
        for item in array {
            if is_truthy_fn(ctx, &fn_val, item)? {
                result.push(item.clone());
            } else {
                break;
            }
        }
        Ok(arr(&result))
    }
}

#[derive(Clone, Copy)]
pub struct DropWhile;
impl Function for DropWhile {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let mut idx = 0;
        for item in array {
            if is_truthy_fn(ctx, &fn_val, item)? {
                idx += 1;
            } else {
                break;
            }
        }
        Ok(arr(&array[idx..]))
    }
}

#[derive(Clone, Copy)]
pub struct Scan;
impl Function for Scan {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let mut acc = args[2].clone();
        let mut results = Vec::with_capacity(array.len());
        for item in array {
            let result_val = apply_fn2(ctx, &fn_val, &acc, item)?;
            acc = result_val;
            results.push(acc.clone());
        }
        Ok(arr(&results))
    }
}

#[derive(Clone, Copy)]
pub struct Foldr;
impl Function for Foldr {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let mut acc = args[2].clone();
        for item in array.iter().rev() {
            acc = apply_fn2(ctx, &fn_val, &acc, item)?;
        }
        Ok(acc)
    }
}

#[derive(Clone, Copy)]
pub struct FindIndex;
impl Function for FindIndex {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let idx = find_index(ctx, &fn_val, array.iter())?;
        Ok(num(idx.map(|i| i as f64).unwrap_or(-1.0)))
    }
}

#[derive(Clone, Copy)]
pub struct FindLast;
impl Function for FindLast {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let result = find_first(ctx, &fn_val, array.iter().rev())?;
        match result {
            Some(v) => Ok(serde_json::Value::from(v)),
            None => Ok(serde_json::Value::from(serde_json::Value::Null)),
        }
    }
}

#[derive(Clone, Copy)]
pub struct FindLastIndex;
impl Function for FindLastIndex {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let idx = find_index(ctx, &fn_val, array.iter().rev())?;
        Ok(num(idx
            .map(|i| (array.len() - 1 - i) as f64)
            .unwrap_or(-1.0)))
    }
}

// ════════════════════════════════════════════════════════════════════
// Additional array operations
// ════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
pub struct Range;
impl Function for Range {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let start = args[0].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        let end = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        let step = if args.len() >= 3 {
            args[2].as_f64().ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "number", "non-number",
            )))?
        } else {
            1.0
        };
        if step == 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-zero step", "zero step",
            )));
        }
        let mut values = Vec::new();
        let mut current = start;
        if step > 0.0 {
            while current < end {
                values.push(serde_json::json!(current));
                current += step;
            }
        } else {
            while current > end {
                values.push(serde_json::json!(current));
                current += step;
            }
        }
        Ok(arr(&values))
    }
}
