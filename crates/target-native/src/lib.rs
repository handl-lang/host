//! # handl-target-native
//!
//! ADR-0002 `Target` plugin that registers the native compilation target.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    OptionSchema, OptionScope, OptionsPlugin, PlatformError, Plugin, PluginContext,
};

/// A `Target` plugin that contributes the `target` option and a native code
/// generation plugin.
pub struct NativeTarget;

impl Plugin for NativeTarget {
    fn id(&self) -> &str {
        "target.native"
    }

    fn name(&self) -> &str {
        "target-native"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<OptionsPlugin>(OptionSchema {
            id: "target".into(),
            scope: OptionScope::Global,
            applies_to: None,
            default: serde_json::Value::String("native".into()),
            possible_values: vec![
                serde_json::Value::String("native".into()),
                serde_json::Value::String("wasm".into()),
            ],
            description: "Compilation target".into(),
        })?;
        ctx.register_plugin(NativeCodeGen)
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// Native code generation plugin placeholder.
struct NativeCodeGen;

impl Plugin for NativeCodeGen {
    fn id(&self) -> &str {
        "codegen.native"
    }

    fn name(&self) -> &str {
        "native-codegen"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
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
