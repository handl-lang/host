use handl_plugin_vm::{Context, Function, GenericFunction};
use handl_plugin_vm_values_json::{Error, NonFunctionValue};

use crate::shared::{
    apply_fn, arr, b, find_first, is_truthy_fn, key_to_string, map_iter, require_function_and_array,
};

// ── flat_map ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct FlatMap;
impl Function<NonFunctionValue> for FlatMap {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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

impl GenericFunction<NonFunctionValue> for FlatMap {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

// ── find ───────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Find;
impl Function<NonFunctionValue> for Find {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (fn_val, array) = require_function_and_array(args)?;
        let result = find_first(ctx, &fn_val, array.iter())?;
        match result {
            Some(v) => Ok(NonFunctionValue::new(v)),
            None => Ok(NonFunctionValue::new(serde_json::Value::Null)),
        }
    }
}

impl GenericFunction<NonFunctionValue> for Find {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

// ── some / every ───────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct SomeFn;
impl Function<NonFunctionValue> for SomeFn {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (fn_val, array) = require_function_and_array(args)?;
        for item in array {
            if is_truthy_fn(ctx, &fn_val, item)? {
                return Ok(b(true));
            }
        }
        Ok(b(false))
    }
}

impl GenericFunction<NonFunctionValue> for SomeFn {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

#[derive(Clone, Copy)]
pub struct Every;
impl Function<NonFunctionValue> for Every {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let (fn_val, array) = require_function_and_array(args)?;
        for item in array {
            if !is_truthy_fn(ctx, &fn_val, item)? {
                return Ok(b(false));
            }
        }
        Ok(b(true))
    }
}

impl GenericFunction<NonFunctionValue> for Every {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

// ── group_by ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct GroupBy;
impl Function<NonFunctionValue> for GroupBy {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
        Ok(NonFunctionValue::new(serde_json::Value::Object(result)))
    }
}

impl GenericFunction<NonFunctionValue> for GroupBy {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

// ── partition ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Partition;
impl Function<NonFunctionValue> for Partition {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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

impl GenericFunction<NonFunctionValue> for Partition {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}

// ── count_by ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct CountBy;
impl Function<NonFunctionValue> for CountBy {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
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
        Ok(NonFunctionValue::new(serde_json::Value::Object(result)))
    }
}

impl GenericFunction<NonFunctionValue> for CountBy {
    fn call_with_types(
        &self,
        args: &[NonFunctionValue],
        _type_args: &[String],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        self.call(args, ctx)
    }
}
