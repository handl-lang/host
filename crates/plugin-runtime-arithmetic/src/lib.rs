//! Arithmetic runtime builtins: `add`, `sub`, `mul`, `div`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use handl_platform::{
    as_number, expect_n, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, PlatformError, Plugin, PluginContext, RuntimeBehavior, RuntimeHost,
    RuntimeRule,
};
use handl_plugin_vm::{Context, Error};
use handl_plugin_vm_values_json::NonFunctionValue;

/// Arithmetic built-in family.
pub struct ArithmeticFamily;

impl RuntimeBehavior for ArithmeticFamily {
    fn kind(&self) -> &str {
        "runtime.arithmetic"
    }

    fn provides(&self) -> &[&'static str] {
        &["add", "sub", "mul", "div"]
    }

    fn call(
        &self,
        name: &str,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let a = as_number(&args[0])?;
        let b = as_number(&args[1])?;
        match name {
            "add" => Ok(NonFunctionValue::new(a + b)),
            "sub" => Ok(NonFunctionValue::new(a - b)),
            "mul" => Ok(NonFunctionValue::new(a * b)),
            "div" => {
                if b == 0.0 {
                    return Err(Error::PrimitiveError("division by zero".to_string()));
                }
                Ok(NonFunctionValue::new(a / b))
            }
            _ => Err(Error::PrimitiveError(format!(
                "unknown arithmetic builtin: {name}"
            ))),
        }
    }
}

/// Registers the arithmetic family with the platform `RuntimeHost`.
pub struct ArithmeticPlugin {
    registered: AtomicBool,
}

impl ArithmeticPlugin {
    /// Create a new `ArithmeticPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for ArithmeticPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ArithmeticPlugin {
    fn id(&self) -> &str {
        "runtime.arithmetic"
    }

    fn name(&self) -> &str {
        "arithmetic"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if !self.registered.load(Ordering::SeqCst) {
            self.registered.store(true, Ordering::SeqCst);
            ctx.contribute::<RuntimeHost>(RuntimeRule::new(Arc::new(ArithmeticFamily)))?;
        }
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}
