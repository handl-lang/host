//! VM module exposing `hello.greet` to the runtime.

use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Error, NonFunctionValue};

use crate::greet;

/// `hello.greet(name)` — returns the canonical Handl hello message.
#[derive(Clone, Copy)]
pub struct HelloGreet;

impl Function<NonFunctionValue> for HelloGreet {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() != 1 {
            return Err(Error::ArgumentCount {
                expected: 1,
                got: args.len(),
            });
        }
        let name = args[0].as_str().ok_or(Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        })?;
        Ok(NonFunctionValue::new(serde_json::Value::String(greet(name))))
    }
}

/// The `hello` library module.
#[derive(Clone, Copy)]
pub struct HelloModule;

impl Module for HelloModule {
    fn name(&self) -> &str {
        "hello"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "greet" => Some(&HelloGreet),
            _ => None,
        }
    }
}
