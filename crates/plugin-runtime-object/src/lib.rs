//! Object runtime builtins: `obj.get`, `obj.keys`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use handl_platform::{
    expect_n, json_kind, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, PlatformError, Plugin, PluginContext, RuntimeBehavior, RuntimeHost,
    RuntimeRule,
};
use handl_plugin_vm::{Context, Error};
use handl_plugin_vm_values_json::NonFunctionValue;
use serde_json::Value as JsonValue;

/// Object built-in family.
pub struct ObjectFamily;

impl RuntimeBehavior for ObjectFamily {
    fn kind(&self) -> &str {
        "runtime.object"
    }

    fn provides(&self) -> &[&'static str] {
        &["obj.get", "obj.keys"]
    }

    fn call(
        &self,
        name: &str,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        match name {
            "obj.get" => {
                expect_n(args, 2)?;
                let obj = args[0].as_object().ok_or(Error::TypeMismatch {
                    expected: "object",
                    got: json_kind(&args[0]),
                })?;
                let key = args[1].as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: json_kind(&args[1]),
                })?;
                Ok(NonFunctionValue::new(
                    obj.get(key).cloned().unwrap_or(JsonValue::Null),
                ))
            }
            "obj.keys" => {
                expect_n(args, 1)?;
                let obj = args[0].as_object().ok_or(Error::TypeMismatch {
                    expected: "object",
                    got: json_kind(&args[0]),
                })?;
                let keys: Vec<JsonValue> =
                    obj.keys().cloned().map(JsonValue::String).collect();
                Ok(NonFunctionValue::new(keys))
            }
            _ => Err(Error::PrimitiveError(format!(
                "unknown object builtin: {name}"
            ))),
        }
    }
}

/// Registers the object family with the platform `RuntimeHost`.
pub struct ObjectPlugin {
    registered: AtomicBool,
}

impl ObjectPlugin {
    /// Create a new `ObjectPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for ObjectPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ObjectPlugin {
    fn id(&self) -> &str {
        "runtime.object"
    }

    fn name(&self) -> &str {
        "object"
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
            ctx.contribute::<RuntimeHost>(RuntimeRule::new(Arc::new(ObjectFamily)))?;
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
