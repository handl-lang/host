use handl_platform::{EvalContext, EvalError, Function};

use crate::shared::{
    apply_fn, arr, b, find_first, is_truthy_fn, key_to_string, map_iter, require_function_and_array,
};

// ── flat_map ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct FlatMap;
impl Function for FlatMap {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_val, array) = require_function_and_array(args)?;
        let mapped = map_iter(ctx, &fn_val, array.iter())?;
        let mut results = Vec::new();
        for item in mapped {
            if let Some(sub) = item.as_array() {
                results.extend(sub.iter().cloned());
            } else {
                results.push(item);
            }
        }
        Ok(arr(&results))
    }
}

// ── find ───────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Find;
impl Function for Find {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_val, array) = require_function_and_array(args)?;
        let result = find_first(ctx, &fn_val, array.iter())?;
        match result {
            Some(v) => Ok(serde_json::Value::from(v)),
            None => Ok(serde_json::Value::from(serde_json::Value::Null)),
        }
    }
}

// ── some / every ───────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct SomeFn;
impl Function for SomeFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_val, array) = require_function_and_array(args)?;
        for item in array {
            if is_truthy_fn(ctx, &fn_val, item)? {
                return Ok(b(true));
            }
        }
        Ok(b(false))
    }
}

#[derive(Clone, Copy)]
pub struct Every;
impl Function for Every {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_val, array) = require_function_and_array(args)?;
        for item in array {
            if !is_truthy_fn(ctx, &fn_val, item)? {
                return Ok(b(false));
            }
        }
        Ok(b(true))
    }
}

// ── group_by ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct GroupBy;
impl Function for GroupBy {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_val, array) = require_function_and_array(args)?;
        let mut groups: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
            std::collections::BTreeMap::new();
        for item in array {
            let key = apply_fn(ctx, &fn_val, item)?;
            let key_str = key_to_string(&key);
            groups.entry(key_str).or_default().push(item.clone());
        }
        let mut result = serde_json::Map::new();
        for (k, v) in groups {
            result.insert(k, serde_json::Value::Array(v));
        }
        Ok(serde_json::Value::from(serde_json::Value::Object(result)))
    }
}

// ── partition ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Partition;
impl Function for Partition {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_val, array) = require_function_and_array(args)?;
        let mut matching = Vec::new();
        let mut rest = Vec::new();
        for item in array {
            if is_truthy_fn(ctx, &fn_val, item)? {
                matching.push(item.clone());
            } else {
                rest.push(item.clone());
            }
        }
        Ok(arr(&[
            serde_json::Value::Array(matching),
            serde_json::Value::Array(rest),
        ]))
    }
}

// ── count_by ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct CountBy;
impl Function for CountBy {
    fn call(
        &self,
        args: &[serde_json::Value],
        ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (fn_val, array) = require_function_and_array(args)?;
        let mut counts: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
        for item in array {
            let key = apply_fn(ctx, &fn_val, item)?;
            let key_str = key_to_string(&key);
            *counts.entry(key_str).or_insert(0.0) += 1.0;
        }
        let mut result = serde_json::Map::new();
        for (k, v) in counts {
            result.insert(k, serde_json::json!(v));
        }
        Ok(serde_json::Value::from(serde_json::Value::Object(result)))
    }
}
