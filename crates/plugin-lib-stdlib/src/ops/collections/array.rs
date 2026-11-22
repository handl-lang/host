use handl_platform::{EvalContext, EvalError, Function};

use super::common::{arr, call_callable, is_truthy_callable, num};

// ════════════════════════════════════════════════════════════════════
// Shared helpers for array functions
// ════════════════════════════════════════════════════════════════════
//
// Callbacks arrive as `{"__arrow__": …}` wire values under the tree-walk
// eval; `super::common::call_callable`/`is_truthy_callable` dispatch them
// through `ctx.try_call_value` (with a legacy `FunctionValue` +
// `eval_expr` fallback).

fn require_array_value(arg: &serde_json::Value) -> Result<&[serde_json::Value], EvalError> {
    arg.as_array()
        .map(Vec::as_slice)
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "array", "non-array",
        )))
}

fn find_first<'a, I>(
    ctx: &mut EvalContext,
    callee: &serde_json::Value,
    iter: I,
) -> Result<Option<serde_json::Value>, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    for item in iter {
        if is_truthy_callable(ctx, callee, item)? {
            return Ok(Some(item.clone()));
        }
    }
    Ok(None)
}

fn find_index<'a, I>(
    ctx: &mut EvalContext,
    callee: &serde_json::Value,
    iter: I,
) -> Result<Option<usize>, EvalError>
where
    I: Iterator<Item = &'a serde_json::Value>,
{
    for (i, item) in iter.enumerate() {
        if is_truthy_callable(ctx, callee, item)? {
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
        let callee = &args[1];
        let mut result = Vec::new();
        for item in array {
            if is_truthy_callable(ctx, callee, item)? {
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
        let callee = &args[1];
        let mut idx = 0;
        for item in array {
            if is_truthy_callable(ctx, callee, item)? {
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
        let callee = &args[1];
        let mut acc = args[2].clone();
        let mut results = Vec::with_capacity(array.len());
        for item in array {
            let call_args = [acc.clone(), item.clone()];
            let result_val = call_callable(ctx, callee, &call_args)?;
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
        let callee = &args[1];
        let mut acc = args[2].clone();
        for item in array.iter().rev() {
            let call_args = [acc.clone(), item.clone()];
            acc = call_callable(ctx, callee, &call_args)?;
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
        let idx = find_index(ctx, &args[1], array.iter())?;
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
        let result = find_first(ctx, &args[1], array.iter().rev())?;
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
        let idx = find_index(ctx, &args[1], array.iter().rev())?;
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
