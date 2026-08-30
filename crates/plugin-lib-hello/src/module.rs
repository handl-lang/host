//! Module exposing `hello.greet` to the runtime.

use handl_platform::{EvalContext, EvalError, Function, Module};

use crate::greet;

/// `hello.greet(name)` — returns the canonical Handl hello message.
#[derive(Clone, Copy)]
pub struct HelloGreet;

impl Function for HelloGreet {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::TypeMismatch(format!(
                "hello.greet: expected 1 argument, got {}",
                args.len()
            )));
        }
        let name = args[0].as_str().ok_or_else(|| {
            EvalError::TypeMismatch("hello.greet: expected string argument".to_string())
        })?;
        Ok(serde_json::Value::String(greet(name)))
    }
}

/// The `hello` library module.
#[derive(Clone, Copy)]
pub struct HelloModule;

impl Module for HelloModule {
    fn name(&self) -> &str {
        "hello"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "greet" => Some(&HelloGreet),
            _ => None,
        }
    }
}
