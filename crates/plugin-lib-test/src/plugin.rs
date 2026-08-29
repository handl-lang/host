//! Plugin registration for the `test` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_vm_common::{ModuleHost, ModuleRule};

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
        ctx.contribute::<ModuleHost>(ModuleRule::new(TestModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(AssertModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(PropertyModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(BenchmarkModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(FuzzModule))?;
        ctx.contribute::<ModuleHost>(ModuleRule::new(SnapshotModule))?;
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
