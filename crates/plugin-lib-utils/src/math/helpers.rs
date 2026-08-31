use handl_platform::EvalError;

pub(crate) use crate::shared::{b, num};

pub(super) fn require_number(arg: &serde_json::Value) -> Result<f64, EvalError> {
    arg.as_f64()
        .ok_or(handl_platform::EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))
}

pub(super) fn require_two_numbers(args: &[serde_json::Value]) -> Result<(f64, f64), EvalError> {
    if args.len() < 2 {
        return Err(handl_platform::EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    let a = require_number(&args[0])?;
    let b = require_number(&args[1])?;
    Ok((a, b))
}

macro_rules! unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl handl_platform::Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut handl_platform::EvalContext,
            ) -> Result<serde_json::Value, handl_platform::EvalError> {
                let arg = args
                    .first()
                    .ok_or(handl_platform::EvalError::TypeMismatch(format!(
                        "expected {} args, got {}",
                        1, 0,
                    )))?;
                let x = require_number(arg)?;
                Ok(num($body(x)))
            }
        }
    };
}

pub(super) use unary_fn;

macro_rules! bool_unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl handl_platform::Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut handl_platform::EvalContext,
            ) -> Result<serde_json::Value, handl_platform::EvalError> {
                let arg = args
                    .first()
                    .ok_or(handl_platform::EvalError::TypeMismatch(format!(
                        "expected {} args, got {}",
                        1, 0,
                    )))?;
                let x = require_number(arg)?;
                Ok(b($body(x)))
            }
        }
    };
}

pub(super) use bool_unary_fn;
