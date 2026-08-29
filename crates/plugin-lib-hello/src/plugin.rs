//! Plugin registration for the `hello` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use std::sync::atomic::{AtomicBool, Ordering};

use super::HelloPlugin;

/// A `Library` plugin that registers the `hello` implementation.
pub struct HelloLibraryPlugin {
    registered: AtomicBool,
}

impl HelloLibraryPlugin {
    /// Create a new `HelloLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for HelloLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for HelloLibraryPlugin {
    fn id(&self) -> &'static str {
        "lib.hello"
    }

    fn name(&self) -> &'static str {
        "lib-hello"
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
            ctx.register_plugin(HelloPlugin::default())
        } else {
            Ok(())
        }
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}
