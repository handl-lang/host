//! # handl-kit-hello
//!
//! Demo kit that extends `kit-canonical` with the `hello` library.

use handl_kit_canonical::CanonicalKit;
use handl_lib_hello::HelloLibraryPlugin;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use std::sync::atomic::{AtomicBool, Ordering};

/// A kit that composes the canonical kit plus the `hello` library.
pub struct HelloKit {
    registered: AtomicBool,
}

impl HelloKit {
    /// Create a new `HelloKit`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for HelloKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for HelloKit {
    fn id(&self) -> &str {
        "kit.hello"
    }

    fn name(&self) -> &str {
        "kit-hello"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);
        ctx.register_plugin(CanonicalKit::new())?;
        ctx.register_plugin(HelloLibraryPlugin::new())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}
