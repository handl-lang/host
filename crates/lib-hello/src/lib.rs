//! # handl-lib-hello
//!
//! Minimal example standard library for the handl runtime.
//!
//! Provides the `hello` library as an ADR-0002 `Library` plugin.

/// Greet `name` with the canonical Handl hello message.
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use std::sync::atomic::{AtomicBool, Ordering};

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
    fn id(&self) -> &str {
        "lib.hello"
    }

    fn name(&self) -> &str {
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

/// The concrete `hello` implementation exposed as a plugin.
pub struct HelloPlugin {
    capabilities: Vec<CapabilityId>,
}

impl Default for HelloPlugin {
    fn default() -> Self {
        Self {
            capabilities: vec!["hello.greet".into()],
        }
    }
}

impl Plugin for HelloPlugin {
    fn id(&self) -> &str {
        "lib.hello.greet"
    }

    fn name(&self) -> &str {
        "hello"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let name = request
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("world");
        Ok(CapabilityResponse(serde_json::Value::String(greet(name))))
    }
}
