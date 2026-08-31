//! Plugin registration for the `test` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_module::{ModuleRule, ModuleRuntime};

use crate::{
    AssertModule, BenchmarkModule, FuzzModule, PropertyModule, SnapshotModule, TestModule,
};

/// A `Library` plugin that registers the `test` implementation.
#[derive(Debug, Default)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.test"
    }

    fn name(&self) -> &'static str {
        "lib-test"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(TestModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(AssertModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(PropertyModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(BenchmarkModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(FuzzModule))?;
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(SnapshotModule))?;
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
