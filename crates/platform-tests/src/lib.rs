//! # handl-platform-tests
//!
//! Integration-test support for the `handl-platform` host: a recording mock
//! plugin, a manifest builder, and a request helper.
//!
//! This crate was removed with the pre-composable platform (`bcf31afa "Remove
//! obsolete crates and files now superseded by the composable platform."`) and
//! is restored here against the current API. Three things did **not** come
//! back, because the API they targeted is gone:
//!
//! - `MockHost` — `handl_platform::DefaultHost` already *is* the mutable
//!   plugin + kit host, so the tests drive it directly instead of shadowing
//!   it.
//! - `MockExtension` — the `Extension` trait and `ExtensionRegistry` no
//!   longer exist anywhere in the workspace; `PluginContext` now carries only
//!   `plugins` and `kits`.
//! - `tests/language_host.rs` — it targeted the removed `LanguageHost` /
//!   `SyntaxNode` and the deleted `handl-plugin-comments` /
//!   `handl-plugin-doc-comments`.
//!
//! [`MockPlugin`] is kept rather than replaced by
//! [`handl_platform::InProcessPlugin`] because it *records* the capabilities
//! it was asked to run, so the tests can assert that dispatch actually
//! reached the plugin and not merely that some response came back.

use std::sync::{Arc, Mutex};

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext, PluginManifest,
};

/// A [`Plugin`] that answers every request with a fixed payload and logs the
/// capability ids it was asked for.
#[derive(Debug, Clone)]
pub struct MockPlugin {
    name: String,
    capabilities: Vec<CapabilityId>,
    output: serde_json::Value,
    calls: Arc<Mutex<Vec<String>>>,
}

impl MockPlugin {
    /// Builds a mock named `name` providing `capabilities`.
    pub fn new(name: impl Into<String>, capabilities: &[&str]) -> Self {
        Self {
            name: name.into(),
            capabilities: capabilities.iter().map(|c| (*c).into()).collect(),
            output: serde_json::Value::Null,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Sets the payload every request is answered with.
    pub fn with_output(mut self, output: serde_json::Value) -> Self {
        self.output = output;
        self
    }

    /// Returns the capability ids this mock was asked to run, in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("mock call log").clone()
    }
}

impl Plugin for MockPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities.contains(capability).then(|| {
            serde_json::json!({
                "plugin": self.name,
                "capability": capability.as_str(),
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        self.calls
            .lock()
            .expect("mock call log")
            .push(request.capability.as_str().to_string());
        Ok(CapabilityResponse(self.output.clone()))
    }
}

/// Builds a manifest for plugin `id` providing `provides`, with no
/// dependencies, conflicts or target declarations.
pub fn mock_manifest(id: &str, provides: &[&str]) -> PluginManifest {
    PluginManifest {
        id: id.to_string(),
        version: "0.0.0".to_string(),
        provides: provides.iter().map(|c| (*c).into()).collect(),
        requires: Vec::new(),
        optional: Vec::new(),
        extends: Vec::new(),
        conflicts: Vec::new(),
        replaces: Vec::new(),
        targets: Vec::new(),
        exclusive: Vec::new(),
        metadata: serde_json::Value::Null,
        ast_kinds: Vec::new(),
    }
}

/// Builds a request for `capability` carrying `payload`.
pub fn request(capability: &str, payload: serde_json::Value) -> CapabilityRequest {
    CapabilityRequest {
        capability: capability.into(),
        payload,
    }
}
