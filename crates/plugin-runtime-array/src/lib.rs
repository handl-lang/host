//! Array runtime builtins: `array.at`, `array.length`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use handl_platform::{
    as_number, expect_n, json_kind, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, PlatformError, Plugin, PluginContext, RuntimeBehavior,
    RuntimeHost, RuntimeRule,
};
use handl_plugin_vm::{Context, Error};
use handl_plugin_vm_values_json::NonFunctionValue;
use serde_json::Value as JsonValue;

/// Array built-in family.
pub struct ArrayFamily;

impl RuntimeBehavior for ArrayFamily {
    fn kind(&self) -> &str {
        "runtime.array"
    }

    fn provides(&self) -> &[&'static str] {
        &["array.at", "array.length"]
    }

    fn call(
        &self,
        name: &str,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        match name {
            "array.at" => {
                expect_n(args, 2)?;
                let arr = args[0].as_array().ok_or(Error::TypeMismatch {
                    expected: "array",
                    got: json_kind(&args[0]),
                })?;
                let idx = as_number(&args[1])? as usize;
                Ok(NonFunctionValue::new(
                    arr.get(idx).cloned().unwrap_or(JsonValue::Null),
                ))
            }
            "array.length" => {
                expect_n(args, 1)?;
                let arr = args[0].as_array().ok_or(Error::TypeMismatch {
                    expected: "array",
                    got: json_kind(&args[0]),
                })?;
                Ok(NonFunctionValue::new(arr.len() as f64))
            }
            _ => Err(Error::PrimitiveError(format!(
                "unknown array builtin: {name}"
            ))),
        }
    }
}

/// Registers the array family with the platform `RuntimeHost`.
pub struct ArrayPlugin {
    registered: AtomicBool,
}

impl ArrayPlugin {
    /// Create a new `ArrayPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for ArrayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ArrayPlugin {
    fn id(&self) -> &str {
        "runtime.array"
    }

    fn name(&self) -> &str {
        "array"
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
            ctx.contribute::<RuntimeHost>(RuntimeRule::new(Arc::new(ArrayFamily)))?;
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
