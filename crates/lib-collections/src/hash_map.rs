//! `collections.hash_map` — key/value dictionary backed by a hash table.
//!
//! Currently implemented as a wrapper around `serde_json::Value::Object`.

use handl_vm::{Context, Function};
use handl_vm_values_json::{Error, FunctionValue, NonFunctionValue};
use serde_json::{Map, Value};
use std::collections::HashSet;

use super::common::{arr, b, num, obj as object_value, require_array, require_object, require_one_array, require_one_object};

// ── shared helpers ────────────────────────────────────────────────

fn require_key(args: &[NonFunctionValue], idx: usize) -> Result<&str, Error> {
    let arg = args.get(idx).ok_or(Error::ArgumentCount {
        expected: idx + 1,
        got: args.len(),
    })?;
    arg.as_str().ok_or(Error::TypeMismatch {
        expected: "string",
        got: "non-string",
    })
}

fn require_one_arg(args: &[NonFunctionValue]) -> Result<&NonFunctionValue, Error> {
    args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: 0,
    })
}

fn require_two_args(
    args: &[NonFunctionValue],
) -> Result<(&NonFunctionValue, &NonFunctionValue), Error> {
    if args.len() < 2 {
        return Err(Error::ArgumentCount {
            expected: 2,
            got: args.len(),
        });
    }
    Ok((&args[0], &args[1]))
}

fn require_three_args(
    args: &[NonFunctionValue],
) -> Result<(&NonFunctionValue, &NonFunctionValue, &NonFunctionValue), Error> {
    if args.len() < 3 {
        return Err(Error::ArgumentCount {
            expected: 3,
            got: args.len(),
        });
    }
    Ok((&args[0], &args[1], &args[2]))
}

fn require_string(arg: &NonFunctionValue) -> Result<&str, Error> {
    arg.as_str().ok_or(Error::TypeMismatch {
        expected: "string",
        got: "non-string",
    })
}

fn require_object_value(arg: &NonFunctionValue) -> Result<&Map<String, Value>, Error> {
    arg.as_object().ok_or(Error::TypeMismatch {
        expected: "object",
        got: "non-object",
    })
}

fn require_function_value(arg: &NonFunctionValue) -> Result<FunctionValue, Error> {
    FunctionValue::from_json(arg.inner()).ok_or(Error::TypeMismatch {
        expected: "function value",
        got: "non-function",
    })
}

fn bind_param(fn_val: &FunctionValue, value: &Value) -> Vec<(String, NonFunctionValue)> {
    fn_val
        .params
        .first()
        .map(|(name, _)| vec![(name.clone(), NonFunctionValue::new(value.clone()))])
        .unwrap_or_default()
}

fn apply_fn(
    ctx: &mut dyn Context<NonFunctionValue>,
    fn_val: &FunctionValue,
    value: &Value,
) -> Result<NonFunctionValue, Error> {
    let vars = bind_param(fn_val, value);
    ctx.eval_expr(&fn_val.body, &vars)
}

fn is_truthy_value(value: &NonFunctionValue) -> bool {
    if let Some(b) = value.as_bool() {
        b
    } else if let Some(n) = value.as_number() {
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
    ctx: &mut dyn Context<NonFunctionValue>,
    fn_val: &FunctionValue,
    value: &Value,
) -> Result<bool, Error> {
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

fn key_to_string(value: &NonFunctionValue) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else if let Some(n) = value.as_number() {
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
        serde_json::to_string(&value.clone().into_inner())
            .unwrap_or_else(|_| "unknown".to_string())
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
    ctx: &mut dyn Context<NonFunctionValue>,
) -> Result<Value, Error> {
    if segments.is_empty() {
        let vars = bind_param(fn_val, &value);
        let result = ctx.eval_expr(&fn_val.body, &vars)?;
        return Ok(result.into_inner());
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

impl Function<NonFunctionValue> for New {
    fn call(
        &self,
        _args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        Ok(object_value(Map::new()))
    }
}

/// `collections.hash_map.from(object: object) → object`
#[derive(Clone, Copy)]
pub struct From;

impl Function<NonFunctionValue> for From {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_object(args)?;
        Ok(object_value(input.clone()))
    }
}

/// `collections.hash_map.to_object(map: hash_map) → object`
#[derive(Clone, Copy)]
pub struct ToObject;

impl Function<NonFunctionValue> for ToObject {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_object(args)?;
        Ok(object_value(input.clone()))
    }
}

// ── inspection ────────────────────────────────────────────────────

/// `collections.hash_map.len(map: hash_map) → number`
#[derive(Clone, Copy)]
pub struct Len;

impl Function<NonFunctionValue> for Len {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_object(args)?;
        Ok(num(input.len() as f64))
    }
}

/// `collections.hash_map.is_empty(map: hash_map) → bool`
#[derive(Clone, Copy)]
pub struct IsEmpty;

impl Function<NonFunctionValue> for IsEmpty {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_object(args)?;
        Ok(b(input.is_empty()))
    }
}

/// `collections.hash_map.keys(map: hash_map) → array<string>`
#[derive(Clone, Copy)]
pub struct Keys;

impl Function<NonFunctionValue> for Keys {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_object(args)?;
        let keys: Vec<Value> = input
            .keys()
            .map(|k| Value::String(k.clone()))
            .collect();
        Ok(arr(&keys))
    }
}

/// `collections.hash_map.values(map: hash_map) → array`
#[derive(Clone, Copy)]
pub struct Values;

impl Function<NonFunctionValue> for Values {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_object(args)?;
        let values: Vec<Value> = input.values().cloned().collect();
        Ok(arr(&values))
    }
}

/// `collections.hash_map.entries(map: hash_map) → array<[string, value]>`
#[derive(Clone, Copy)]
pub struct Entries;

impl Function<NonFunctionValue> for Entries {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_object(args)?;
        let entries: Vec<Value> = input
            .iter()
            .map(|(k, v)| Value::Array(vec![Value::String(k.clone()), v.clone()]))
            .collect();
        Ok(arr(&entries))
    }
}

/// `collections.hash_map.size(map: hash_map) → number`
#[derive(Clone, Copy)]
pub struct Size;

impl Function<NonFunctionValue> for Size {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_one_object(args)?;
        Ok(num(input.len() as f64))
    }
}

// ── access ────────────────────────────────────────────────────────

/// `collections.hash_map.get(map: hash_map, key: string, default?: value) → value`
#[derive(Clone, Copy)]
pub struct Get;

impl Function<NonFunctionValue> for Get {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_object(args, 0)?;
        let key = require_key(args, 1)?;

        match input.get(key) {
            Some(value) => Ok(NonFunctionValue::new(value.clone())),
            None => match args.get(2) {
                Some(default) => Ok(NonFunctionValue::new(default.inner().clone())),
                None => Ok(NonFunctionValue::new(Value::Null)),
            },
        }
    }
}

/// `collections.hash_map.get_or(map: hash_map, key: string, default: value) → value`
#[derive(Clone, Copy)]
pub struct GetOr;

impl Function<NonFunctionValue> for GetOr {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (obj_arg, key_arg, default_arg) = require_three_args(args)?;
        let obj = require_object_value(obj_arg)?;
        let key = require_string(key_arg)?;
        match obj.get(key) {
            Some(Value::Null) | None => Ok(default_arg.clone()),
            Some(v) => Ok(NonFunctionValue::new(v.clone())),
        }
    }
}

/// `collections.hash_map.has(map: hash_map, key: string) → bool`
#[derive(Clone, Copy)]
pub struct Has;

impl Function<NonFunctionValue> for Has {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_object(args, 0)?;
        let key = require_key(args, 1)?;
        Ok(b(input.contains_key(key)))
    }
}

/// `collections.hash_map.deep_get(value: value, path: string) → value`
#[derive(Clone, Copy)]
pub struct DeepGet;

impl Function<NonFunctionValue> for DeepGet {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() < 2 {
            return Err(Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }
        let path = args[1].as_str().ok_or(Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        })?;
        let mut current = args[0].inner().clone();
        for segment in path.split('.') {
            if segment.is_empty() {
                continue;
            }
            current = match &current {
                Value::Object(obj) => {
                    obj.get(segment).cloned().unwrap_or(Value::Null)
                }
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
        Ok(NonFunctionValue::new(current))
    }
}

/// `collections.hash_map.path_or(value: value, path: string, default: value) → value`
#[derive(Clone, Copy)]
pub struct PathOr;

impl Function<NonFunctionValue> for PathOr {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (value_arg, path_arg, default_arg) = require_three_args(args)?;
        let path = require_string(path_arg)?;
        let mut current = value_arg.inner().clone();
        for segment in path.split('.') {
            if segment.is_empty() {
                continue;
            }
            current = match &current {
                Value::Object(obj) => {
                    obj.get(segment).cloned().unwrap_or(Value::Null)
                }
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
            Ok(NonFunctionValue::new(current))
        }
    }
}

// ── mutation ──────────────────────────────────────────────────────

/// `collections.hash_map.set(map: hash_map, key: string, value: value) → hash_map`
#[derive(Clone, Copy)]
pub struct Set;

impl Function<NonFunctionValue> for Set {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let input = require_object(args, 0)?;
        let key = require_key(args, 1)?;
        let value = args.get(2).ok_or(Error::ArgumentCount {
            expected: 3,
            got: args.len(),
        })?;

        let mut map = input.clone();
        map.insert(key.to_string(), value.inner().clone());
        Ok(object_value(map))
    }
}

/// `collections.hash_map.remove(map: hash_map, key: string) → hash_map`
#[derive(Clone, Copy)]
pub struct Remove;

impl Function<NonFunctionValue> for Remove {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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

impl Function<NonFunctionValue> for Update {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (obj_arg, key_arg, fn_arg) = require_three_args(args)?;
        let obj = require_object_value(obj_arg)?;
        let key = require_string(key_arg)?;
        let fn_val = require_function_value(fn_arg)?;
        let current = obj.get(key).cloned().unwrap_or(Value::Null);
        let new_value = apply_fn(ctx, &fn_val, &current)?;
        let mut result = obj.clone();
        result.insert(key.to_string(), new_value.into_inner());
        Ok(object_value(result))
    }
}

/// `collections.hash_map.set_path(map: hash_map, path: string, value: value) → value`
#[derive(Clone, Copy)]
pub struct SetPath;

impl Function<NonFunctionValue> for SetPath {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (obj_arg, path_arg, value_arg) = require_three_args(args)?;
        let path = require_string(path_arg)?;
        let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        let result = set_path_recursive(
            obj_arg.inner().clone(),
            &segments,
            value_arg.inner().clone(),
        );
        Ok(NonFunctionValue::new(result))
    }
}

/// `collections.hash_map.update_path(map: hash_map, path: string, fn: function) → value`
#[derive(Clone, Copy)]
pub struct UpdatePath;

impl Function<NonFunctionValue> for UpdatePath {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (obj_arg, path_arg, fn_arg) = require_three_args(args)?;
        let path = require_string(path_arg)?;
        let fn_val = require_function_value(fn_arg)?;
        let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        let result = update_path_recursive(obj_arg.inner().clone(), &segments, &fn_val, ctx)?;
        Ok(NonFunctionValue::new(result))
    }
}

// ── transformation ────────────────────────────────────────────────

/// `collections.hash_map.map_values(fn: function, map: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct MapValues;

impl Function<NonFunctionValue> for MapValues {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (fn_arg, obj_arg) = require_two_args(args)?;
        let fn_val = require_function_value(fn_arg)?;
        let obj = require_object_value(obj_arg)?;
        let mut result = Map::new();
        for (k, v) in obj {
            let new_value = apply_fn(ctx, &fn_val, v)?;
            result.insert(k.clone(), new_value.into_inner());
        }
        Ok(object_value(result))
    }
}

/// `collections.hash_map.map_keys(fn: function, map: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct MapKeys;

impl Function<NonFunctionValue> for MapKeys {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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

impl Function<NonFunctionValue> for FilterKeys {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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

impl Function<NonFunctionValue> for FilterValues {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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

impl Function<NonFunctionValue> for Invert {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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

/// `collections.hash_map.clone_deep(value: value) → value`
#[derive(Clone, Copy)]
pub struct CloneDeep;

impl Function<NonFunctionValue> for CloneDeep {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let arg = require_one_arg(args)?;
        Ok(NonFunctionValue::new(arg.inner().clone()))
    }
}

// ── selection ─────────────────────────────────────────────────────

/// `collections.hash_map.pick(map: hash_map, keys: array<string>) → hash_map`
#[derive(Clone, Copy)]
pub struct Pick;

impl Function<NonFunctionValue> for Pick {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() < 2 {
            return Err(Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
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
        Ok(NonFunctionValue::new(Value::Object(result)))
    }
}

/// `collections.hash_map.omit(map: hash_map, keys: array<string>) → hash_map`
#[derive(Clone, Copy)]
pub struct Omit;

impl Function<NonFunctionValue> for Omit {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() < 2 {
            return Err(Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }
        let obj = require_object_value(&args[0])?;
        let keys = require_array(args, 1)?;
        let omit_set: HashSet<&str> =
            keys.iter().filter_map(|v| v.as_str()).collect();
        let mut result = Map::new();
        for (k, v) in obj {
            if !omit_set.contains(k.as_str()) {
                result.insert(k.clone(), v.clone());
            }
        }
        Ok(NonFunctionValue::new(Value::Object(result)))
    }
}

// ── combination ───────────────────────────────────────────────────

/// `collections.hash_map.merge(a: hash_map, b: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct Merge;

impl Function<NonFunctionValue> for Merge {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (a_arg, b_arg) = require_two_args(args)?;
        let a = require_object_value(a_arg)?;
        let b = require_object_value(b_arg)?;
        let mut result = a.clone();
        for (k, v) in b {
            result.insert(k.clone(), v.clone());
        }
        Ok(NonFunctionValue::new(Value::Object(result)))
    }
}

/// `collections.hash_map.deep_merge(a: hash_map, b: hash_map) → hash_map`
#[derive(Clone, Copy)]
pub struct DeepMerge;

impl Function<NonFunctionValue> for DeepMerge {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (a_arg, b_arg) = require_two_args(args)?;
        let a = require_object_value(a_arg)?;
        let b = require_object_value(b_arg)?;
        let merged = deep_merge_values(
            &Value::Object(a.clone()),
            &Value::Object(b.clone()),
        );
        Ok(NonFunctionValue::new(merged))
    }
}

/// `collections.hash_map.from_entries(entries: array<[string, value]>) → hash_map`
#[derive(Clone, Copy)]
pub struct FromEntries;

impl Function<NonFunctionValue> for FromEntries {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let entries = require_one_array(args)?;
        let mut result = Map::new();
        for entry in entries {
            let pair = entry.as_array().ok_or(Error::TypeMismatch {
                expected: "array pair [key, value]",
                got: "non-array",
            })?;
            if pair.len() < 2 {
                return Err(Error::TypeMismatch {
                    expected: "pair with at least 2 elements",
                    got: "short pair",
                });
            }
            let key = pair[0].as_str().ok_or(Error::TypeMismatch {
                expected: "string key",
                got: "non-string",
            })?;
            result.insert(key.to_string(), pair[1].clone());
        }
        Ok(object_value(result))
    }
}
