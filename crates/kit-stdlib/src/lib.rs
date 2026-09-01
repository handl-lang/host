//! # handl-kit-stdlib
//!
//! Kit that extends the canonical kit with the standard library modules:
//! `hello`, `fn` (combinators: map, filter, reduce, compose, ...),
//! `collections` (vec, list, queue, stack, array operations), and more.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_collections::LibraryPlugin as CollectionsLibraryPlugin;
use handl_plugin_lib_hello::HelloLibraryPlugin;
use handl_plugin_lib_utils::LibraryPlugin as UtilsLibraryPlugin;
use std::sync::atomic::{AtomicBool, Ordering};

/// A kit that composes the standard library plugins.
///
/// This kit registers:
/// - `hello.*` — greeting library
/// - `fn.*` — function combinators (map, filter, reduce, compose, pipe, ...)
/// - `collections.*` — vec, list, queue, stack, hash_map, array operations
pub struct StdlibKit {
    registered: AtomicBool,
}

impl StdlibKit {
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for StdlibKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for StdlibKit {
    fn id(&self) -> &str {
        "kit.stdlib"
    }

    fn name(&self) -> &str {
        "kit-stdlib"
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
        ctx.register_plugin(handl_plugin_module::ModuleRuntime::default())?;
        ctx.register_plugin(HelloLibraryPlugin::new())?;
        ctx.register_plugin(UtilsLibraryPlugin)?;
        ctx.register_plugin(CollectionsLibraryPlugin)?;
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
