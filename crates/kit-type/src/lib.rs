//! # handl-kit-type
//!
//! Thin kit that curates type-related plugins and re-exports the
//! Normalization-by-Evaluation type-level VM from `handl-plugin-typecheck-nbe`.

pub use handl_plugin_typecheck_nbe::*;

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use std::sync::atomic::{AtomicBool, Ordering};

/// A kit that curates the typecheck plugins.
pub struct TypeKit {
    registered: AtomicBool,
}

impl TypeKit {
    /// Create a new `TypeKit`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for TypeKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TypeKit {
    fn id(&self) -> &str {
        "kit.type"
    }

    fn name(&self) -> &str {
        "kit-type"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);
        // TODO: register the concrete typecheck plugin implementations.
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
