//! Rust `Function` impls for the `option` module — the handls (handlers)
//! for the `op`/`handl` declarations in `option.hnl`.
//!
//! All 15 functions are FFI atoms: they manipulate `serde_json::Value`
//! option values (using the `_tag` convention: `{"_tag": "Some", "value":
//! ...}` / `{"_tag": "None"}`) and call `apply_fn` from `crate::shared`.
//! They cannot be expressed as pure handl arrows without losing the exact
//! `TypeMismatch` error messages the tests rely on. The `op`/`handl`
//! declaration form (ADR-0077 Phase 2) declares the interface in
//! `option.hnl`; these impls provide the logic and are registered in an
//! [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).

use handl_platform::{EvalContext, EvalError, Function};
use serde_json::{Value as JsonValue, json};

use crate::shared::{apply_fn, is_truthy_value, require_function_value};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn as_option_tag(value: &JsonValue) -> Option<&str> {
    value.get("_tag")?.as_str()
}

fn is_some(value: &JsonValue) -> bool {
    as_option_tag(value) == Some("Some")
}

fn is_none(value: &JsonValue) -> bool {
    as_option_tag(value) == Some("None")
}

fn require_option(arg: &serde_json::Value) -> Result<(), EvalError> {
    let inner = arg;
    if as_option_tag(inner).is_some() {
        Ok(())
    } else {
        Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "option", "non-option",
        )))
    }
}

fn get_value(arg: &serde_json::Value) -> Option<&JsonValue> {
    arg.get("value")
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct SomeFn;
impl Function for SomeFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?;
        Ok(serde_json::Value::from(json!({
            "_tag": "Some",
            "value": arg.clone(),
        })))
    }
}

#[derive(Clone, Copy)]
pub struct NoneFn;
impl Function for NoneFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(serde_json::Value::from(json!({"_tag": "None"})))
    }
}

// ---------------------------------------------------------------------------
// Predicates
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct IsSome;
impl Function for IsSome {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?;
        Ok(serde_json::Value::from(json!(is_some(arg))))
    }
}

#[derive(Clone, Copy)]
pub struct IsNone;
impl Function for IsNone {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?;
        Ok(serde_json::Value::from(json!(is_none(arg))))
    }
}

// ---------------------------------------------------------------------------
// Unwrap
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Unwrap;
impl Function for Unwrap {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?;
        require_option(arg)?;
        if let Some(value) = get_value(arg) {
            Ok(serde_json::Value::from(value.clone()))
        } else {
            Err(EvalError::Other("called unwrap on None".to_string()))
        }
    }
}

#[derive(Clone, Copy)]
pub struct UnwrapOr;
impl Function for UnwrapOr {
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
        require_option(&args[0])?;
        if let Some(value) = get_value(&args[0]) {
            Ok(serde_json::Value::from(value.clone()))
        } else {
            Ok(args[1].clone())
        }
    }
}

#[derive(Clone, Copy)]
pub struct UnwrapOrElse;
impl Function for UnwrapOrElse {
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
        require_option(&args[0])?;
        if let Some(value) = get_value(&args[0]) {
            Ok(serde_json::Value::from(value.clone()))
        } else {
            let fn_val = require_function_value(&args[1])?;
            ctx.eval_expr(&fn_val.body, &[])
        }
    }
}

// ---------------------------------------------------------------------------
// Map / FlatMap / AndThen / Filter
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Map;
impl Function for Map {
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
        require_option(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let value = get_value(&args[0]).cloned().unwrap_or(JsonValue::Null);
        if as_option_tag(&args[0]) == Some("Some") {
            let mapped = apply_fn(ctx, &fn_val, &value)?;
            Ok(serde_json::Value::from(json!({
                "_tag": "Some",
                "value": mapped.clone(),
            })))
        } else {
            Ok(serde_json::Value::from(json!({"_tag": "None"})))
        }
    }
}

#[derive(Clone, Copy)]
pub struct FlatMap;
impl Function for FlatMap {
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
        require_option(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        let value = get_value(&args[0]).cloned().unwrap_or(JsonValue::Null);
        if as_option_tag(&args[0]) == Some("Some") {
            apply_fn(ctx, &fn_val, &value)
        } else {
            Ok(serde_json::Value::from(json!({"_tag": "None"})))
        }
    }
}

#[derive(Clone, Copy)]
pub struct Filter;
impl Function for Filter {
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
        require_option(&args[0])?;
        let fn_val = require_function_value(&args[1])?;
        if as_option_tag(&args[0]) == Some("Some") {
            let value = get_value(&args[0]).cloned().unwrap_or(JsonValue::Null);
            let pred = apply_fn(ctx, &fn_val, &value)?;
            if is_truthy_value(&pred) {
                Ok(args[0].clone())
            } else {
                Ok(serde_json::Value::from(json!({"_tag": "None"})))
            }
        } else {
            Ok(serde_json::Value::from(json!({"_tag": "None"})))
        }
    }
}

// ---------------------------------------------------------------------------
// MapOr
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct MapOr;
impl Function for MapOr {
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
        require_option(&args[0])?;
        let fn_val = require_function_value(&args[2])?;
        let value = get_value(&args[0]).cloned().unwrap_or(JsonValue::Null);
        if as_option_tag(&args[0]) == Some("Some") {
            apply_fn(ctx, &fn_val, &value)
        } else {
            Ok(args[1].clone())
        }
    }
}

// ---------------------------------------------------------------------------
// Or / OrElse / And / Xor
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct Or;
impl Function for Or {
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
        if as_option_tag(&args[0]) == Some("Some") {
            Ok(args[0].clone())
        } else {
            Ok(args[1].clone())
        }
    }
}

#[derive(Clone, Copy)]
pub struct OrElse;
impl Function for OrElse {
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
        if as_option_tag(&args[0]) == Some("Some") {
            Ok(args[0].clone())
        } else {
            let fn_val = require_function_value(&args[1])?;
            ctx.eval_expr(&fn_val.body, &[])
        }
    }
}

#[derive(Clone, Copy)]
pub struct And;
impl Function for And {
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
        if as_option_tag(&args[0]) == Some("Some") {
            Ok(args[1].clone())
        } else {
            Ok(serde_json::Value::from(json!({"_tag": "None"})))
        }
    }
}

#[derive(Clone, Copy)]
pub struct Xor;
impl Function for Xor {
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
        let a_is_some = is_some(&args[0]);
        let b_is_some = is_some(&args[1]);
        if a_is_some && !b_is_some {
            Ok(args[0].clone())
        } else if !a_is_some && b_is_some {
            Ok(args[1].clone())
        } else {
            Ok(serde_json::Value::from(json!({"_tag": "None"})))
        }
    }
}
