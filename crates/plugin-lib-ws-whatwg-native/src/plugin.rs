//! Plugin registration for the `ws-whatwg-native` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};

/// A `Library` plugin that registers the `ws-whatwg-native` implementation.
#[derive(Debug, Default)]
pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.ws-whatwg-native"
    }

    fn name(&self) -> &'static str {
        "lib-ws-whatwg-native"
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
