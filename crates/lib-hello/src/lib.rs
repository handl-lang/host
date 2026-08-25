//! # handl-lib-hello
//!
//! Minimal example standard library for the handl runtime.
//!
//! Provides `hello.greet(name)` which returns a friendly greeting string.

use handl_tooling::Library;
use handl_vm::{Context, Error, Function};
use handl_vm_common::{Module, ModuleHost};
use handl_vm_values_json::NonFunctionValue;

static GREET: Greet = Greet;

/// The `hello` module — a single `greet` function.
#[derive(Clone, Copy)]
pub struct HelloModule;

impl Module for HelloModule {
    fn name(&self) -> &str {
        "hello"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "greet" => Some(&GREET),
            _ => None,
        }
    }
}

struct Greet;

impl Function<NonFunctionValue> for Greet {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        let name = args.first().and_then(|v| v.as_str()).unwrap_or("world");
        Ok(NonFunctionValue::new(format!("Hello, {name}!")))
    }
}

/// A `Library` wrapper that installs the `hello` module into a `ModuleHost`.
#[derive(Clone, Copy)]
pub struct HelloLib;

impl Library for HelloLib {
    fn name(&self) -> &str {
        "hello"
    }

    fn install(&self, host: ModuleHost) -> ModuleHost {
        host.with(HelloModule)
    }
}
