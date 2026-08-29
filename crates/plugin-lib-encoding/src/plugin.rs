//! Plugin registration for the `encoding` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};

/// A `Library` plugin that registers the `encoding` implementation.
#[derive(Debug, Default)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.encoding"
    }

    fn name(&self) -> &'static str {
        "lib-encoding"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        // TODO: register concrete library implementation
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
