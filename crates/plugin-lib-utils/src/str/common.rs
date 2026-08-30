use handl_platform::{EvalContext, EvalError, Function};


pub(super) fn require_string(arg: &serde_json::Value) -> Result<&str, EvalError> {
    arg.as_str().ok_or(handl_platform::EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))
}

pub(super) fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args.first().ok_or(handl_platform::EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
    require_string(arg)
}

pub(super) fn require_two_strings(args: &[serde_json::Value]) -> Result<(&str, &str), EvalError> {
    if args.len() < 2 {
        return Err(handl_platform::EvalError::TypeMismatch(format!("expected {} args, got {}", 2, args.len(),)));
    }
    let a = require_string(&args[0])?;
    let b = require_string(&args[1])?;
    Ok((a, b))
}

pub(super) fn s(value: impl Into<String>) -> serde_json::Value {
    serde_json::Value::from(value.into())
}

pub(super) fn b(value: bool) -> serde_json::Value {
    serde_json::Value::from(value)
}

pub(super) fn num(value: f64) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(value))
}

macro_rules! string_unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl handl_platform::Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut handl_platform::EvalContext,
            ) -> Result<serde_json::Value, handl_platform::EvalError> {
                let input = $crate::r#str::common::require_one_string(args)?;
                Ok($crate::r#str::common::s($body(input)))
            }
        }
    };
}
pub(super) use string_unary_fn;

macro_rules! string_predicate_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl handl_platform::Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut handl_platform::EvalContext,
            ) -> Result<serde_json::Value, handl_platform::EvalError> {
                let (input, pattern) = $crate::r#str::common::require_two_strings(args)?;
                Ok($crate::r#str::common::b($body(input, pattern)))
            }
        }
    };
}
pub(super) use string_predicate_fn;
