use handl_platform::{EvalContext, EvalError, Function};
use handl_platform::Module;

use crate::dedent::dedent;

/// The `dedent` module — whitespace stripping utilities.
#[derive(Clone, Copy)]
pub struct DedentModule;

impl Module for DedentModule {
    fn name(&self) -> &str {
        "dedent"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "dedent" => Some(&Dedent),
            _ => None,
        }
    }
}

/// `dedent.dedent(text)` — strips common leading whitespace from all lines.
#[derive(Clone, Copy)]
pub struct Dedent;

impl Function for Dedent {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or(EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0,)))?;
        let text = arg.as_str().ok_or(EvalError::TypeMismatch(format!("expected {}, got {}", "string", "non-string",)))?;

        Ok(serde_json::Value::from(dedent(text)))
    }
}
