//! Plugin registration for the `prim` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_module::{ModuleRule, ModuleRuntime};

use crate::PrimModule;

/// A `Library` plugin that registers the `prim` module.
#[derive(Debug, Default)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.prim"
    }

    fn name(&self) -> &'static str {
        "lib-prim"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(PrimModule))?;
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
