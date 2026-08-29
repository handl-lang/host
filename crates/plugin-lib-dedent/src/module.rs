use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Error, NonFunctionValue};

use crate::dedent::dedent;

/// The `dedent` module — whitespace stripping utilities.
#[derive(Clone, Copy)]
pub struct DedentModule;

impl Module for DedentModule {
    fn name(&self) -> &str {
        "dedent"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "dedent" => Some(&Dedent),
            _ => None,
        }
    }
}

/// `dedent.dedent(text)` — strips common leading whitespace from all lines.
#[derive(Clone, Copy)]
pub struct Dedent;

impl Function<NonFunctionValue> for Dedent {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let arg = args.first().ok_or(Error::ArgumentCount {
            expected: 1,
            got: 0,
        })?;
        let text = arg.as_str().ok_or(Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        })?;

        Ok(NonFunctionValue::new(dedent(text)))
    }
}
