//! Boolean runtime builtins: `and`, `or`, `not`, `eq`, `lt`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use handl_platform::{
    as_bool, as_number, expect_n, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, PlatformError, Plugin, PluginContext, RuntimeBehavior,
    RuntimeHost, RuntimeRule,
};
use handl_vm::{Context, Error};
use handl_vm_values_json::NonFunctionValue;

/// Boolean built-in family.
pub struct BooleanFamily;

impl RuntimeBehavior for BooleanFamily {
    fn kind(&self) -> &str {
        "runtime.boolean"
    }

    fn provides(&self) -> &[&'static str] {
        &["and", "or", "not", "eq", "lt"]
    }

    fn call(
        &self,
        name: &str,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        match name {
            "and" | "or" => {
                expect_n(args, 2)?;
                let a = as_bool(&args[0])?;
                let b = as_bool(&args[1])?;
                Ok(NonFunctionValue::new(if name == "and" {
                    a && b
                } else {
                    a || b
                }))
            }
            "not" => {
                expect_n(args, 1)?;
                let a = as_bool(&args[0])?;
                Ok(NonFunctionValue::new(!a))
            }
            "eq" => {
                expect_n(args, 2)?;
                Ok(NonFunctionValue::new(args[0].inner() == args[1].inner()))
            }
            "lt" => {
                expect_n(args, 2)?;
                let a = as_number(&args[0])?;
                let b = as_number(&args[1])?;
                Ok(NonFunctionValue::new(a < b))
            }
            _ => Err(Error::PrimitiveError(format!(
                "unknown boolean builtin: {name}"
            ))),
        }
    }
}

/// Registers the boolean family with the platform `RuntimeHost`.
pub struct BooleanPlugin {
    registered: AtomicBool,
}

impl BooleanPlugin {
    /// Create a new `BooleanPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for BooleanPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BooleanPlugin {
    fn id(&self) -> &str {
        "runtime.boolean"
    }

    fn name(&self) -> &str {
        "boolean"
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
            ctx.contribute::<RuntimeHost>(RuntimeRule::new(Arc::new(BooleanFamily)))?;
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
