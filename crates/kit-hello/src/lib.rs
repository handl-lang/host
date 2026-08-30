//! # handl-kit-hello
//!
//! Demo kit that extends the canonical kit with the `hello` library.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_hello::HelloLibraryPlugin;
use std::sync::atomic::{AtomicBool, Ordering};

/// A kit that composes the `hello` library plugin.
///
/// Note: The language operations (lexer, parser, evaluator) are now provided
/// by the data-driven `handl-kit-canonical::Kit` (backed by `handl-runtime`).
/// This plugin only registers the `hello` library module.
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
        // Register ModuleRuntime so library plugins can register their modules.
        ctx.register_plugin(handl_plugin_module::ModuleRuntime::default())?;
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
