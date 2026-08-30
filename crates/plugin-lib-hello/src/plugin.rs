//! Plugin registration for the `hello` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_module::{ModuleRuntime, ModuleRule};
use std::sync::atomic::{AtomicBool, Ordering};

use super::{HelloModule, HelloPlugin};

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
            // Register the `hello` VM module so `hello.greet(...)` resolves
            // during VM evaluation, and the capability plugin for the
            // platform capability dispatch path.
            ctx.contribute::<ModuleRuntime>(ModuleRule::new(HelloModule))?;
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
