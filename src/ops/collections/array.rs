use crate::function_value::FunctionValue;
use handl_platform::{EvalContext, EvalError, Function};

use super::common::{arr, b, num, require_one_array, require_two_arrays};

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

// ════════════════════════════════════════════════════════════════════
// Unary array-function macro
// ════════════════════════════════════════════════════════════════════

macro_rules! array_unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut EvalContext,
            ) -> Result<serde_json::Value, EvalError> {
                let input = require_one_array(args)?;
                Ok($body(input))
            }
        }
    };
}

// ════════════════════════════════════════════════════════════════════
// Basic operations
// ════════════════════════════════════════════════════════════════════

array_unary_fn!(
    Length,
    |input: &[serde_json::Value]| num(input.len() as f64)
);
array_unary_fn!(Reverse, |input: &[serde_json::Value]| {
    let reversed: Vec<serde_json::Value> = input.iter().rev().cloned().collect();
    arr(&reversed)
});
array_unary_fn!(First, |input: &[serde_json::Value]| match input.first() {
    Some(v) => serde_json::Value::from(v.clone()),
    None => serde_json::Value::from(serde_json::Value::Null),
});
array_unary_fn!(Last, |input: &[serde_json::Value]| match input.last() {
    Some(v) => serde_json::Value::from(v.clone()),
    None => serde_json::Value::from(serde_json::Value::Null),
});
array_unary_fn!(Flatten, |input: &[serde_json::Value]| {
    let mut flat = Vec::new();
    for item in input {
        match item {
            serde_json::Value::Array(nested) => flat.extend(nested.iter().cloned()),
            other => flat.push(other.clone()),
        }
    }
    arr(&flat)
});

#[derive(Clone, Copy)]
pub struct DeepFlatten;
impl Function for DeepFlatten {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        fn deep_flat(items: &[serde_json::Value], out: &mut Vec<serde_json::Value>) {
            for item in items {
                match item {
                    serde_json::Value::Array(nested) => deep_flat(nested, out),
                    other => out.push(other.clone()),
                }
            }
        }
        let mut flat = Vec::new();
        deep_flat(input, &mut flat);
        Ok(arr(&flat))
    }
}

#[derive(Clone, Copy)]
pub struct Concat;
impl Function for Concat {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_arrays(args)?;
        let combined: Vec<serde_json::Value> = a.iter().chain(b.iter()).cloned().collect();
        Ok(arr(&combined))
    }
}

#[derive(Clone, Copy)]
pub struct Contains;
impl Function for Contains {
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
        let needle = &args[1];
        let found = array.iter().any(|item| item == needle);
        Ok(b(found))
    }
}

#[derive(Clone, Copy)]
pub struct IndexOf;
impl Function for IndexOf {
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
        let needle = &args[1];
        let idx = array
            .iter()
            .position(|item| item == needle)
            .map(|i| i as f64)
            .unwrap_or(-1.0);
        Ok(num(idx))
    }
}

#[derive(Clone, Copy)]
pub struct Slice;
impl Function for Slice {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let start = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        let end = args[2].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if start < 0.0 || end < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let start = start as usize;
        let end = end as usize;
        let end = end.min(array.len());
        let start = start.min(end);
        Ok(arr(&array[start..end]))
    }
}

#[derive(Clone, Copy)]
pub struct At;
impl Function for At {
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
        let index = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if index < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        match array.get(index as usize) {
            Some(v) => Ok(serde_json::Value::from(v.clone())),
            None => Ok(serde_json::Value::from(serde_json::Value::Null)),
        }
    }
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

#[derive(Clone, Copy)]
pub struct Pop;
impl Function for Pop {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let array = require_one_array(args)?;
        if array.is_empty() {
            return Ok(arr(&[]));
        }
        Ok(arr(&array[..array.len() - 1]))
    }
}

#[derive(Clone, Copy)]
pub struct Shift;
impl Function for Shift {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let array = require_one_array(args)?;
        if array.is_empty() {
            return Ok(arr(&[]));
        }
        Ok(arr(&array[1..]))
    }
}

#[derive(Clone, Copy)]
pub struct Fill;
impl Function for Fill {
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
        let count = args[0].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if count < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let value = args[1].clone();
        let result: Vec<serde_json::Value> = (0..count as usize).map(|_| value.clone()).collect();
        Ok(arr(&result))
    }
}

// ════════════════════════════════════════════════════════════════════
// Iteration operations
// ════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
pub struct Take;
impl Function for Take {
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
        let n = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if n < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let n = (n as usize).min(array.len());
        Ok(arr(&array[..n]))
    }
}

#[derive(Clone, Copy)]
pub struct Drop;
impl Function for Drop {
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
        let n = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if n < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let n = (n as usize).min(array.len());
        Ok(arr(&array[n..]))
    }
}

array_unary_fn!(Enumerate, |input: &[serde_json::Value]| {
    let pairs: Vec<serde_json::Value> = input
        .iter()
        .enumerate()
        .map(|(i, v)| serde_json::json!([i, v.clone()]))
        .collect();
    arr(&pairs)
});

array_unary_fn!(Unzip, |input: &[serde_json::Value]| {
    let mut firsts = Vec::new();
    let mut seconds = Vec::new();
    for item in input {
        if let Some(pair) = item.as_array() {
            if pair.len() >= 2 {
                firsts.push(pair[0].clone());
                seconds.push(pair[1].clone());
            }
        }
    }
    arr(&[
        serde_json::Value::Array(firsts),
        serde_json::Value::Array(seconds),
    ])
});

#[derive(Clone, Copy)]
pub struct Intersperse;
impl Function for Intersperse {
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
        let sep = args[1].clone();
        let mut result = Vec::with_capacity(array.len().saturating_mul(2));
        for (i, item) in array.iter().enumerate() {
            if i > 0 {
                result.push(sep.clone());
            }
            result.push(item.clone());
        }
        Ok(arr(&result))
    }
}

#[derive(Clone, Copy)]
pub struct Intercalate;
impl Function for Intercalate {
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
        let sep = require_array_value(&args[1])?;
        let mut result = Vec::new();
        for (i, item) in array.iter().enumerate() {
            if i > 0 {
                result.extend(sep.iter().cloned());
            }
            if let Some(nested) = item.as_array() {
                result.extend(nested.iter().cloned());
            } else {
                result.push(item.clone());
            }
        }
        Ok(arr(&result))
    }
}

array_unary_fn!(Dedupe, |input: &[serde_json::Value]| {
    let mut result = Vec::new();
    for item in input {
        if result.last() != Some(item) {
            result.push(item.clone());
        }
    }
    arr(&result)
});

#[derive(Clone, Copy)]
pub struct Prepend;
impl Function for Prepend {
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
        let mut result = Vec::with_capacity(array.len() + 1);
        result.push(args[1].clone());
        result.extend(array.iter().cloned());
        Ok(arr(&result))
    }
}

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

#[derive(Clone, Copy)]
pub struct Windows;
impl Function for Windows {
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
        let size = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if size < 1.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "positive number", "non-positive number",
            )));
        }
        let size = size as usize;
        if size > array.len() {
            return Ok(arr(&[]));
        }
        let windows: Vec<serde_json::Value> = array
            .windows(size)
            .map(|w| serde_json::Value::Array(w.to_vec()))
            .collect();
        Ok(arr(&windows))
    }
}

#[derive(Clone, Copy)]
pub struct Transpose;
impl Function for Transpose {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let array = require_array_value(args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?)?;
        if array.is_empty() {
            return Ok(arr(&[]));
        }
        let rows: Vec<&[serde_json::Value]> = array
            .iter()
            .map(|item| {
                item.as_array()
                    .map(|a| a.as_slice())
                    .unwrap_or(std::slice::from_ref(item))
            })
            .collect();
        let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if max_cols == 0 {
            return Ok(arr(&[]));
        }
        let mut transposed = Vec::with_capacity(max_cols);
        for col in 0..max_cols {
            let column: Vec<serde_json::Value> =
                rows.iter().filter_map(|r| r.get(col).cloned()).collect();
            transposed.push(serde_json::Value::Array(column));
        }
        Ok(arr(&transposed))
    }
}

#[derive(Clone, Copy)]
pub struct InsertAt;
impl Function for InsertAt {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }
        let array = require_array_value(&args[0])?;
        let index = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if index < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let index = (index as usize).min(array.len());
        let value = args[2].clone();
        let mut result = Vec::with_capacity(array.len() + 1);
        result.extend_from_slice(&array[..index]);
        result.push(value);
        result.extend_from_slice(&array[index..]);
        Ok(arr(&result))
    }
}

#[derive(Clone, Copy)]
pub struct RemoveAt;
impl Function for RemoveAt {
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
        let index = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if index < 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "non-negative number", "negative number",
            )));
        }
        let index = index as usize;
        if index >= array.len() {
            return Ok(arr(array));
        }
        let mut result = Vec::with_capacity(array.len() - 1);
        result.extend_from_slice(&array[..index]);
        result.extend_from_slice(&array[index + 1..]);
        Ok(arr(&result))
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

#[derive(Clone, Copy)]
pub struct Zip;
impl Function for Zip {
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
        let a = require_array_value(&args[0])?;
        let b = require_array_value(&args[1])?;
        let len = a.len().min(b.len());
        let pairs: Vec<serde_json::Value> = (0..len)
            .map(|i| serde_json::json!([a[i].clone(), b[i].clone()]))
            .collect();
        Ok(arr(&pairs))
    }
}

#[derive(Clone, Copy)]
pub struct Chunk;
impl Function for Chunk {
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
        let input = require_array_value(&args[0])?;
        let size = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
        if size <= 0.0 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "positive chunk size", "non-positive chunk size",
            )));
        }
        let size = size as usize;
        let chunks: Vec<serde_json::Value> = input
            .chunks(size)
            .map(|chunk| serde_json::Value::Array(chunk.to_vec()))
            .collect();
        Ok(arr(&chunks))
    }
}

#[derive(Clone, Copy)]
pub struct Compact;
impl Function for Compact {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        let result: Vec<serde_json::Value> =
            input.iter().filter(|v| !v.is_null()).cloned().collect();
        Ok(arr(&result))
    }
}

#[derive(Clone, Copy)]
pub struct Pluck;
impl Function for Pluck {
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
        let key = args[1].as_str().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))?;
        let result: Vec<serde_json::Value> = array
            .iter()
            .map(|item| {
                item.as_object()
                    .and_then(|obj| obj.get(key))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null)
            })
            .collect();
        Ok(arr(&result))
    }
}
