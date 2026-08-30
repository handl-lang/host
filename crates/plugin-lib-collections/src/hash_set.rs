//! `collections.hash_set` — unordered set of unique values.
//!
//! Implemented as a deduplicated array of `serde_json::Value`.
//!
//! Also provides set-theoretic operations on arrays (`unique`, `union`,
//! `intersection`, `difference`, `symmetric_difference`, `without`, `unique_by`).

use std::collections::HashSet;

use handl_platform::{EvalContext, EvalError, Function};
use crate::function_value::FunctionValue;

use super::common::{arr, b, num, require_array, require_one_array, require_two_arrays};

fn has_value(input: &[serde_json::Value], target: &serde_json::Value) -> bool {
    input.iter().any(|item| item == target)
}

/// Serialize a `serde_json::Value` to a string for use as a `HashSet` key.
fn value_key(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "n:".to_string(),
        serde_json::Value::Bool(b) => format!("b:{b}"),
        serde_json::Value::Number(n) => format!("n:{n}"),
        serde_json::Value::String(s) => format!("s:{s}"),
        other => serde_json::to_string(other).unwrap_or_else(|_| "?".to_string()),
    }
}

/// Deduplicate a slice while preserving first-occurrence order.
fn dedup_ordered(items: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let key = value_key(item);
        if seen.insert(key) {
            result.push(item.clone());
        }
    }
    result
}

fn require_function_value(arg: &serde_json::Value) -> Result<FunctionValue, EvalError> {
    FunctionValue::from_json(arg).ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "function value", "non-function",)))
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

/// `collections.hash_set.new() → set`
#[derive(Clone, Copy)]
pub struct New;

impl Function for New {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(arr(&[]))
    }
}

/// `collections.hash_set.from(array: array) → set`
#[derive(Clone, Copy)]
pub struct From;

impl Function for From {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        let mut values: Vec<serde_json::Value> = Vec::new();
        for item in input {
            if !has_value(&values, item) {
                values.push(item.clone());
            }
        }
        Ok(arr(&values))
    }
}

/// `collections.hash_set.to_array(set: set) → array`
#[derive(Clone, Copy)]
pub struct ToArray;

impl Function for ToArray {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        Ok(arr(input))
    }
}

/// `collections.hash_set.len(set: set) → number`
#[derive(Clone, Copy)]
pub struct Len;

impl Function for Len {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        Ok(num(input.len() as f64))
    }
}

/// `collections.hash_set.is_empty(set: set) → bool`
#[derive(Clone, Copy)]
pub struct IsEmpty;

impl Function for IsEmpty {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        Ok(b(input.is_empty()))
    }
}

/// `collections.hash_set.add(set: set, value: value) → set`
#[derive(Clone, Copy)]
pub struct Add;

impl Function for Add {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_array(args, 0)?;
        let value = args.get(1).ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)))?;
        let target = value;
        let mut values = input.to_vec();
        if !has_value(&values, target) {
            values.push(target.clone());
        }
        Ok(arr(&values))
    }
}

/// `collections.hash_set.remove(set: set, value: value) → set`
#[derive(Clone, Copy)]
pub struct Remove;

impl Function for Remove {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_array(args, 0)?;
        let value = args.get(1).ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)))?;
        let target = value;
        let mut values = input.to_vec();
        if let Some(pos) = values.iter().position(|item| item == target) {
            values.remove(pos);
        }
        Ok(arr(&values))
    }
}

/// `collections.hash_set.has(set: set, value: value) → bool`
#[derive(Clone, Copy)]
pub struct Has;

impl Function for Has {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_array(args, 0)?;
        let value = args.get(1).ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)))?;
        Ok(b(has_value(input, value)))
    }
}

/// `collections.hash_set.unique(array: array) → set`
#[derive(Clone, Copy)]
pub struct Unique;

impl Function for Unique {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_array(args)?;
        Ok(arr(&dedup_ordered(input)))
    }
}

/// `collections.hash_set.union(a: set, b: set) → set`
#[derive(Clone, Copy)]
pub struct Union;

impl Function for Union {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_arrays(args)?;
        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::with_capacity(a.len() + b.len());
        for item in a.iter().chain(b.iter()) {
            let key = value_key(item);
            if seen.insert(key) {
                result.push(item.clone());
            }
        }
        Ok(arr(&result))
    }
}

/// `collections.hash_set.intersection(a: set, b: set) → set`
#[derive(Clone, Copy)]
pub struct Intersection;

impl Function for Intersection {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_arrays(args)?;
        let b_set: HashSet<String> = b.iter().map(value_key).collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::new();
        for item in a {
            let key = value_key(item);
            if b_set.contains(&key) && seen.insert(key) {
                result.push(item.clone());
            }
        }
        Ok(arr(&result))
    }
}

/// `collections.hash_set.difference(a: set, b: set) → set`
#[derive(Clone, Copy)]
pub struct Difference;

impl Function for Difference {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_arrays(args)?;
        let b_set: HashSet<String> = b.iter().map(value_key).collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::new();
        for item in a {
            let key = value_key(item);
            if !b_set.contains(&key) && seen.insert(key) {
                result.push(item.clone());
            }
        }
        Ok(arr(&result))
    }
}

/// `collections.hash_set.symmetric_difference(a: set, b: set) → set`
#[derive(Clone, Copy)]
pub struct SymmetricDifference;

impl Function for SymmetricDifference {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a, b) = require_two_arrays(args)?;
        let a_set: HashSet<String> = a.iter().map(value_key).collect();
        let b_set: HashSet<String> = b.iter().map(value_key).collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::new();
        for item in a {
            let key = value_key(item);
            if !b_set.contains(&key) && seen.insert(key) {
                result.push(item.clone());
            }
        }
        for item in b {
            let key = value_key(item);
            if !a_set.contains(&key) && seen.insert(key) {
                result.push(item.clone());
            }
        }
        Ok(arr(&result))
    }
}

/// `collections.hash_set.without(array: array, ...values) → set`
#[derive(Clone, Copy)]
pub struct Without;

impl Function for Without {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let array = require_array(args, 0)?;
        let excludes: HashSet<String> = args[1..].iter().map(|v| value_key(v)).collect();
        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::new();
        for item in array {
            let key = value_key(item);
            if !excludes.contains(&key) && seen.insert(key) {
                result.push(item.clone());
            }
        }
        Ok(arr(&result))
    }
}

/// `collections.hash_set.unique_by(array: array, fn: function) → set`
#[derive(Clone, Copy)]
pub struct UniqueBy;

impl Function for UniqueBy {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
        }
        let array = require_array(args, 0)?;
        let fn_val = require_function_value(&args[1])?;
        let mut seen: HashSet<String> = HashSet::new();
        let mut result = Vec::new();
        for item in array {
            let vars = bind_param(&fn_val, item);
            let key = ctx.eval_expr(&fn_val.body, &vars)?;
            let key_str = value_key(&key);
            if seen.insert(key_str) {
                result.push(item.clone());
            }
        }
        Ok(arr(&result))
    }
}
