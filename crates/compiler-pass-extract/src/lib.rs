//! # handl-compiler-pass-extract
//!
//! `ExtractPass` adds a deterministic `extracted` marker to the JSON
//! payload and can be registered as a `handl-platform` `Plugin`.

use handl_compiler::{CompilerError, Pass, PassHost};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

mod stage;
pub use stage::*;

/// Extract pass: marks the payload with `{"extracted": true}`.
pub struct ExtractPass;

impl ExtractPass {
    pub const fn new() -> Self {
        Self
    }
}

impl Pass for ExtractPass {
    fn id(&self) -> &str {
        "extract"
    }

    fn run(&self, input: Value) -> Result<Value, CompilerError> {
        let mut obj = match input {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        obj.insert("extracted".to_string(), Value::Bool(true));
        obj.insert("stage".to_string(), Value::String(self.id().to_string()));
        Ok(Value::Object(obj))
    }
}

/// Plugin wrapper for `ExtractPass`.
pub struct ExtractPlugin {
    pass: ExtractPass,
    capabilities: Vec<CapabilityId>,
}

impl ExtractPlugin {
    pub fn new() -> Self {
        Self {
            pass: ExtractPass::new(),
            capabilities: vec!["extract".into()],
        }
    }
}

impl Default for ExtractPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ExtractPlugin {
    fn name(&self) -> &str {
        "handl-compiler-pass-extract"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({"command": "extract", "description": "Extract fields from parsed input"}))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let output = self
            .pass
            .run(request.payload.clone())
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;
        Ok(CapabilityResponse(output))
    }
}

/// Default `PassHost` pre-registered with `ExtractPass`.
pub fn default_pass_host() -> PassHost {
    let mut host = PassHost::new();
    host.register(ExtractPass::new());
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{DefaultHost, Host, PluginRegistry};

    #[test]
    fn extract_pass_adds_extracted_marker() {
        let out = ExtractPass::new()
            .run(serde_json::json!({"hello": true}))
            .unwrap();
        assert!(out.get("hello").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("extracted").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn extract_plugin_runs_via_default_host() {
        let mut host = DefaultHost::new();
        host.register_plugin(ExtractPlugin::new());

        let mut plugins = PluginRegistry::new();
        let mut kits = handl_platform::KitRegistry::new();
        let mut extensions = handl_platform::ExtensionRegistry::new();
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };

        let resp = host
            .plugins()
            .resolve(&"extract".into())
            .unwrap()
            .run(
                &CapabilityRequest {
                    capability: "extract".into(),
                    payload: serde_json::json!({}),
                },
                &mut ctx,
            )
            .unwrap();
        assert!(resp.0.get("extracted").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn default_host_runs_extract() {
        let out = default_pass_host().run(serde_json::json!({})).unwrap();
        assert!(out.get("extracted").and_then(|v| v.as_bool()).unwrap_or(false));
    }
}
