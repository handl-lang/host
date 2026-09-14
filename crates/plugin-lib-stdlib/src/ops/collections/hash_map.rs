//! `collections.hash_map` — key/value dictionary backed by a hash table.
//!
//! Currently implemented as a wrapper around `serde_json::Value::Object`.

use crate::function_value::FunctionValue;
use handl_platform::{EvalContext, EvalError, Function};
use serde_json::{Map, Value};
use std::collections::HashSet;

use super::common::{
    num, obj as object_value, require_array, require_object, require_one_array, require_one_object,
};

// ── shared helpers ────────────────────────────────────────────────

fn require_key(args: &[serde_json::Value], idx: usize) -> Result<&str, EvalError> {
    let arg = args.get(idx).ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        idx + 1,
        args.len(),
    )))?;
    arg.as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))
}

fn require_one_arg(args: &[serde_json::Value]) -> Result<&serde_json::Value, EvalError> {
    args.first().ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        1, 0,
    )))
}

fn require_two_args(
    args: &[serde_json::Value],
) -> Result<(&serde_json::Value, &serde_json::Value), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    Ok((&args[0], &args[1]))
}

fn require_three_args(
    args: &[serde_json::Value],
) -> Result<(&serde_json::Value, &serde_json::Value, &serde_json::Value), EvalError> {
    if args.len() < 3 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            3,
            args.len(),
        )));
    }
    Ok((&args[0], &args[1], &args[2]))
}

fn require_string(arg: &serde_json::Value) -> Result<&str, EvalError> {
    arg.as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))
}

fn require_object_value(arg: &serde_json::Value) -> Result<&Map<String, Value>, EvalError> {
    arg.as_object().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "object", "non-object",
    )))
}

fn require_function_value(arg: &serde_json::Value) -> Result<FunctionValue, EvalError> {
    FunctionValue::from_json(arg).ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "function value", "non-function",
    )))
}

fn bind_param(fn_val: &FunctionValue, value: &Value) -> Vec<(String, serde_json::Value)> {
    fn_val
        .params
        .first()
        .map(|(name, _)| vec![(name.clone(), serde_json::Value::from(value.clone()))])
        .unwrap_or_default()
}

fn apply_fn(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    value: &Value,
) -> Result<serde_json::Value, EvalError> {
    let vars = bind_param(fn_val, value);
    ctx.eval_expr(&fn_val.body, &vars)
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

fn is_truthy_fn(
    ctx: &mut EvalContext,
    fn_val: &FunctionValue,
    value: &Value,
) -> Result<bool, EvalError> {
    let result = apply_fn(ctx, fn_val, value)?;
    Ok(is_truthy_value(&result))
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
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
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "unknown".to_string()),
    }
}

fn key_to_string(value: &serde_json::Value) -> String {
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

fn deep_merge_values(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Object(a_obj), Value::Object(b_obj)) => {
            let mut result = a_obj.clone();
            for (k, v) in b_obj {
                if let Some(existing) = result.get(k) {
                    result.insert(k.clone(), deep_merge_values(existing, v));
                } else {
                    result.insert(k.clone(), v.clone());
                }
            }
            Value::Object(result)
        }
        (_, b) => b.clone(),
    }
}

fn set_path_recursive(value: Value, segments: &[&str], new_value: Value) -> Value {
    if segments.is_empty() {
        return new_value;
    }
    let segment = segments[0];
    let rest = &segments[1..];
    let mut map = match value {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    let current = map.get(segment).cloned().unwrap_or(Value::Null);
    let updated = set_path_recursive(current, rest, new_value);
    map.insert(segment.to_string(), updated);
    Value::Object(map)
}

fn update_path_recursive(
    value: Value,
    segments: &[&str],
    fn_val: &FunctionValue,
    ctx: &mut EvalContext,
) -> Result<Value, EvalError> {
    if segments.is_empty() {
        let vars = bind_param(fn_val, &value);
        let result = ctx.eval_expr(&fn_val.body, &vars)?;
        return Ok(result);
    }
    let segment = segments[0];
    let rest = &segments[1..];
    let mut map = match value {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    let current = map.get(segment).cloned().unwrap_or(Value::Null);
    let updated = update_path_recursive(current, rest, fn_val, ctx)?;
    map.insert(segment.to_string(), updated);
    Ok(Value::Object(map))
}

// ── constructors ──────────────────────────────────────────────────

/// `collections.hash_map.new() → object`
#[derive(Clone, Copy)]
pub struct New;

impl Function for New {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(object_value(Map::new()))
    }
}

/// `collections.hash_map.from(object: object) → object`
#[derive(Clone, Copy)]
pub struct From;

impl Function for From {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_object(args)?;
        Ok(object_value(input.clone()))
    }
}

/// `collections.hash_map.to_object(map: hash_map) → object`
#[derive(Clone, Copy)]
pub struct ToObject;

impl Function for ToObject {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_object(args)?;
        Ok(object_value(input.clone()))
    }
}

// ── inspection ────────────────────────────────────────────────────

/// `collections.hash_map.size(map: hash_map) → number`
#[derive(Clone, Copy)]
pub struct Size;

impl Function for Size {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_one_object(args)?;
        Ok(num(input.len() as f64))
    }
}

// ── access ────────────────────────────────────────────────────────

/// `collections.hash_map.get(map: hash_map, key: string, default?: value) → value`
#[derive(Clone, Copy)]
pub struct Get;

impl Function for Get {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_object(args, 0)?;
        let key = require_key(args, 1)?;

        match input.get(key) {
            Some(value) => Ok(serde_json::Value::from(value.clone())),
            None => match args.get(2) {
                Some(default) => Ok(serde_json::Value::from(default.clone())),
                None => Ok(serde_json::Value::from(Value::Null)),
            },
        }
    }
}

/// `collections.hash_map.get_or(map: hash_map, key: string, default: value) → value`
#[derive(Clone, Copy)]
pub struct GetOr;

impl Function for GetOr {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (obj_arg, key_arg, default_arg) = require_three_args(args)?;
        let obj = require_object_value(obj_arg)?;
        let key = require_string(key_arg)?;
        match obj.get(key) {
            Some(Value::Null) | None => Ok(default_arg.clone()),
            Some(v) => Ok(serde_json::Value::from(v.clone())),
        }
    }
}

/// `collections.hash_map.deep_get(value: value, path: string) → value`
#[derive(Clone, Copy)]
pub struct DeepGet;

impl Function for DeepGet {
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
        let path = args[1].as_str().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))?;
        let mut current = args[0].clone();
        for segment in path.split('.') {
            if segment.is_empty() {
                continue;
            }
            current = match &current {
                Value::Object(obj) => obj.get(segment).cloned().unwrap_or(Value::Null),
                Value::Array(arr) => {
                    if let Ok(idx) = segment.parse::<usize>() {
                        arr.get(idx).cloned().unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                }
                _ => Value::Null,
            };
            if current.is_null() {
                break;
            }
        }
        Ok(serde_json::Value::from(current))
    }
}

/// `collections.hash_map.path_or(value: value, path: string, default: value) → value`
#[derive(Clone, Copy)]
pub struct PathOr;

impl Function for PathOr {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (value_arg, path_arg, default_arg) = require_three_args(args)?;
        let path = require_string(path_arg)?;
        let mut current = value_arg.clone();
        for segment in path.split('.') {
            if segment.is_empty() {
                continue;
            }
            current = match &current {
                Value::Object(obj) => obj.get(segment).cloned().unwrap_or(Value::Null),
                Value::Array(arr) => {
                    if let Ok(idx) = segment.parse::<usize>() {
                        arr.get(idx).cloned().unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                }
                _ => Value::Null,
            };
            if current.is_null() {
                break;
            }
        }
        if current.is_null() {
            Ok(default_arg.clone())
        } else {
            Ok(serde_json::Value::from(current))
        }
    }
}

// ── mutation ──────────────────────────────────────────────────────

/// `collections.hash_map.set(map: hash_map, key: string, value: value) → hash_map`
#[derive(Clone, Copy)]
pub struct Set;

impl Function for Set {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_object(args, 0)?;
        let key = require_key(args, 1)?;
        let value = args.get(2).ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            3,
            args.len(),
        )))?;

        let mut map = input.clone();
        map.insert(key.to_string(), value.clone());
        Ok(object_value(map))
    }
}

/// `collections.hash_map.remove(map: hash_map, key: string) → hash_map`
#[derive(Clone, Copy)]
pub struct Remove;

impl Function for Remove {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let input = require_object(args, 0)?;
        let key = require_key(args, 1)?;
        let mut map = input.clone();
        map.remove(key);
        Ok(object_value(map))
    }
}

/// `collections.hash_map.update(map: hash_map, key: string, fn: function) → hash_map`
#[derive(Clone, Copy)]
pub struct Update;

impl Function for Update {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (obj_arg, key_arg, fn_arg) = require_three_args(args)?;
        let obj = require_object_value(obj_arg)?;
        let key = require_string(key_arg)?;
        let fn_val = require_function_value(fn_arg)?;
        let current = obj.get(key).cloned().unwrap_or(Value::Null);
        let new_value = apply_fn(ctx, &fn_val, &current)?;
        let mut result = obj.clone();
        result.insert(key.to_string(), new_value);
        Ok(object_value(result))
    }
}

/// `collections.hash_map.set_path(map: hash_map, path: string, value: value) → value`
#[derive(Clone, Copy)]
pub struct SetPath;

impl Function for SetPath {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (obj_arg, path_arg, value_arg) = require_three_args(args)?;
        let path = require_string(path_arg)?;
        let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        let result = set_path_recursive(obj_arg.clone(), &segments, value_arg.clone());
        Ok(serde_json::Value::from(result))
    }
}

/// `collections.hash_map.update_path(map: hash_map, path: string, fn: function) → value`
#[derive(Clone, Copy)]
pub struct UpdatePath;

impl Function for UpdatePath {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (obj_arg, path_arg, fn_arg) = require_three_args(args)?;
        let path = require_string(path_arg)?;
        let fn_val = require_function_value(fn_arg)?;
        let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        let result = update_path_recursive(obj_arg.clone(), &segments, &fn_val, ctx)?;
        Ok(serde_json::Value::from(result))
    }
}

// ── transformation ────────────────────────────────────────────────

/// `collections.hash_map.map_values(fn: function, map: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct MapValues;

impl Function for MapValues {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_arg, obj_arg) = require_two_args(args)?;
        let fn_val = require_function_value(fn_arg)?;
        let obj = require_object_value(obj_arg)?;
        let mut result = Map::new();
        for (k, v) in obj {
            let new_value = apply_fn(ctx, &fn_val, v)?;
            result.insert(k.clone(), new_value);
        }
        Ok(object_value(result))
    }
}

/// `collections.hash_map.map_keys(fn: function, map: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct MapKeys;

impl Function for MapKeys {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_arg, obj_arg) = require_two_args(args)?;
        let fn_val = require_function_value(fn_arg)?;
        let obj = require_object_value(obj_arg)?;
        let mut result = Map::new();
        for (k, v) in obj {
            let new_key = apply_fn(ctx, &fn_val, &Value::String(k.clone()))?;
            let key_str = key_to_string(&new_key);
            result.insert(key_str, v.clone());
        }
        Ok(object_value(result))
    }
}

/// `collections.hash_map.filter_keys(fn: function, map: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct FilterKeys;

impl Function for FilterKeys {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_arg, obj_arg) = require_two_args(args)?;
        let fn_val = require_function_value(fn_arg)?;
        let obj = require_object_value(obj_arg)?;
        let mut result = Map::new();
        for (k, v) in obj {
            if is_truthy_fn(ctx, &fn_val, &Value::String(k.clone()))? {
                result.insert(k.clone(), v.clone());
            }
        }
        Ok(object_value(result))
    }
}

/// `collections.hash_map.filter_values(fn: function, map: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct FilterValues;

impl Function for FilterValues {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_arg, obj_arg) = require_two_args(args)?;
        let fn_val = require_function_value(fn_arg)?;
        let obj = require_object_value(obj_arg)?;
        let mut result = Map::new();
        for (k, v) in obj {
            if is_truthy_fn(ctx, &fn_val, v)? {
                result.insert(k.clone(), v.clone());
            }
        }
        Ok(object_value(result))
    }
}

/// `collections.hash_map.invert(map: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct Invert;

impl Function for Invert {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = require_one_arg(args)?;
        let obj = require_object_value(arg)?;
        let mut result = Map::new();
        for (k, v) in obj {
            let new_key = value_to_string(v);
            result.insert(new_key, Value::String(k.clone()));
        }
        Ok(object_value(result))
    }
}

// ── selection ─────────────────────────────────────────────────────

/// `collections.hash_map.pick(map: hash_map, keys: array<string>) → hash_map`
#[derive(Clone, Copy)]
pub struct Pick;

impl Function for Pick {
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
        let obj = require_object_value(&args[0])?;
        let keys = require_array(args, 1)?;
        let mut result = Map::new();
        for key_val in keys {
            if let Some(key) = key_val.as_str()
                && let Some(v) = obj.get(key)
            {
                result.insert(key.to_string(), v.clone());
            }
        }
        Ok(serde_json::Value::from(Value::Object(result)))
    }
}

/// `collections.hash_map.omit(map: hash_map, keys: array<string>) → hash_map`
#[derive(Clone, Copy)]
pub struct Omit;

impl Function for Omit {
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
        let obj = require_object_value(&args[0])?;
        let keys = require_array(args, 1)?;
        let omit_set: HashSet<&str> = keys.iter().filter_map(|v| v.as_str()).collect();
        let mut result = Map::new();
        for (k, v) in obj {
            if !omit_set.contains(k.as_str()) {
                result.insert(k.clone(), v.clone());
            }
        }
        Ok(serde_json::Value::from(Value::Object(result)))
    }
}

// ── combination ───────────────────────────────────────────────────

/// `collections.hash_map.merge(a: hash_map, b: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct Merge;

impl Function for Merge {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a_arg, b_arg) = require_two_args(args)?;
        let a = require_object_value(a_arg)?;
        let b = require_object_value(b_arg)?;
        let mut result = a.clone();
        for (k, v) in b {
            result.insert(k.clone(), v.clone());
        }
        Ok(serde_json::Value::from(Value::Object(result)))
    }
}

/// `collections.hash_map.deep_merge(a: hash_map, b: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct DeepMerge;

impl Function for DeepMerge {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (a_arg, b_arg) = require_two_args(args)?;
        let a = require_object_value(a_arg)?;
        let b = require_object_value(b_arg)?;
        let merged = deep_merge_values(&Value::Object(a.clone()), &Value::Object(b.clone()));
        Ok(serde_json::Value::from(merged))
    }
}

/// `collections.hash_map.from_entries(entries: array<[string, value]>) → hash_map`
#[derive(Clone, Copy)]
pub struct FromEntries;

impl Function for FromEntries {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let entries = require_one_array(args)?;
        let mut result = Map::new();
        for entry in entries {
            let pair = entry
                .as_array()
                .map(Vec::as_slice)
                .ok_or(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "array pair [key, value]", "non-array",
                )))?;
            if pair.len() < 2 {
                return Err(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "pair with at least 2 elements", "short pair",
                )));
            }
            let key = pair[0].as_str().ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "string key", "non-string",
            )))?;
            result.insert(key.to_string(), pair[1].clone());
        }
        Ok(object_value(result))
    }
}
