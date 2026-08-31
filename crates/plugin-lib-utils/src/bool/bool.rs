use handl_platform::{EvalContext, EvalError, Function, Module};

/// The `bool` module — boolean utility functions.
///
/// Provides functions callable as `bool.and(a, b)`, `bool.or(a, b)`,
/// etc. in handl expressions.
#[derive(Clone, Copy)]
pub struct BoolModule;

impl Module for BoolModule {
    fn name(&self) -> &str {
        "bool"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "and" => Some(&And),
            "or" => Some(&Or),
            "not" => Some(&Not),
            "xor" => Some(&Xor),
            "nand" => Some(&Nand),
            "nor" => Some(&Nor),
            "implies" => Some(&Implies),
            "eq" => Some(&Eq),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![
            ("and", &And),
            ("or", &Or),
            ("not", &Not),
            ("xor", &Xor),
            ("nand", &Nand),
            ("nor", &Nor),
            ("implies", &Implies),
            ("eq", &Eq),
        ]
    }
}

fn require_bool(arg: &serde_json::Value) -> Result<bool, EvalError> {
    arg.as_bool()
        .ok_or(EvalError::TypeMismatch("expected boolean".to_string()))
}

fn require_one_bool(args: &[serde_json::Value]) -> Result<bool, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(
        "expected 1 argument, got 0".to_string(),
    ))?;
    require_bool(arg)
}

fn require_two_bools(args: &[serde_json::Value]) -> Result<(bool, bool), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected 2 args, got {}",
            args.len()
        )));
    }
    let a = require_bool(&args[0])?;
    let b = require_bool(&args[1])?;
    Ok((a, b))
}

fn to_bool(value: bool) -> serde_json::Value {
    serde_json::Value::from(value)
}

macro_rules! bool_unary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut EvalContext,
            ) -> Result<serde_json::Value, EvalError> {
                let input = require_one_bool(args)?;
                Ok(to_bool($body(input)))
            }
        }
    };
}

macro_rules! bool_binary_fn {
    ($struct_name:ident, $body:expr) => {
        #[derive(Clone, Copy)]
        pub struct $struct_name;
        impl Function for $struct_name {
            fn call(
                &self,
                args: &[serde_json::Value],
                _ctx: &mut EvalContext,
            ) -> Result<serde_json::Value, EvalError> {
                let (a, b) = require_two_bools(args)?;
                Ok(to_bool($body(a, b)))
            }
        }
    };
}

bool_unary_fn!(Not, |a: bool| !a);
bool_binary_fn!(And, |a: bool, b: bool| a && b);
bool_binary_fn!(Or, |a: bool, b: bool| a || b);
bool_binary_fn!(Xor, |a: bool, b: bool| a ^ b);
bool_binary_fn!(Nand, |a: bool, b: bool| !(a && b));
bool_binary_fn!(Nor, |a: bool, b: bool| !(a || b));
bool_binary_fn!(Implies, |a: bool, b: bool| !a || b);
bool_binary_fn!(Eq, |a: bool, b: bool| a == b);

// NOTE: ADR 0040 gap — short-circuit (lazy) `and`/`or` are not yet
// supported by the new non-generic `Module`/`Function` traits. The
// eager `And`/`Or` implementations above are used instead. When the
// new system adds lazy evaluation support, these can be restored.
