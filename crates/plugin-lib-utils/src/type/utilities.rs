
use handl_platform::{EvalContext, EvalError, Function};

fn require_one_arg(args: &[serde_json::Value]) -> Result<&serde_json::Value, EvalError> {
    args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))
}

fn require_two_args(
    args: &[serde_json::Value],
) -> Result<(&serde_json::Value, &serde_json::Value), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
    }
    Ok((&args[0], &args[1]))
}

#[derive(Clone, Copy)]
pub struct Identity;
impl Function for Identity {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = require_one_arg(args)?;
        Ok(arg.clone())
    }
}

#[derive(Clone, Copy)]
pub struct Default;
impl Function for Default {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (value, fallback) = require_two_args(args)?;
        if value.is_null() {
            Ok(fallback.clone())
        } else {
            Ok(value.clone())
        }
    }
}

#[derive(Clone, Copy)]
pub struct Coalesce;
impl Function for Coalesce {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        for arg in args {
            if !arg.is_null() {
                return Ok(arg.clone());
            }
        }
        Ok(serde_json::Value::from(serde_json::Value::Null))
    }
}
