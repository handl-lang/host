//! # handl-compiler-pass-resolve
//!
//! `ResolvePass` adds a deterministic `resolved` marker to the JSON
//! payload and can be registered as a `handl-platform` `Plugin`.

use handl_compiler::{CompilerError, Pass, PassHost};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

mod stage;
pub use stage::*;

/// Resolve pass: marks the payload with `{"resolved": true}`.
pub struct ResolvePass;

impl ResolvePass {
    pub const fn new() -> Self {
        Self
    }
}

impl Pass for ResolvePass {
    fn id(&self) -> &str {
        "resolve"
    }

    fn run(&self, input: Value) -> Result<Value, CompilerError> {
        let mut obj = match input {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        obj.insert("resolved".to_string(), Value::Bool(true));
        obj.insert("stage".to_string(), Value::String(self.id().to_string()));
        Ok(Value::Object(obj))
    }
}

/// Plugin wrapper for `ResolvePass`.
pub struct ResolvePlugin {
    pass: ResolvePass,
    capabilities: Vec<CapabilityId>,
}

impl ResolvePlugin {
    pub fn new() -> Self {
        Self {
            pass: ResolvePass::new(),
            capabilities: vec!["resolve".into()],
        }
    }
}

impl Default for ResolvePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ResolvePlugin {
    fn name(&self) -> &str {
        "handl-compiler-pass-resolve"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({"command": "resolve", "description": "Resolve imports and schemas"}))
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

/// Default `PassHost` pre-registered with `ResolvePass`.
pub fn default_pass_host() -> PassHost {
    let mut host = PassHost::new();
    host.register(ResolvePass::new());
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{DefaultHost, Host, PluginRegistry};

    #[test]
    fn resolve_pass_adds_resolved_marker() {
        let out = ResolvePass::new()
            .run(serde_json::json!({"hello": true}))
            .unwrap();
        assert!(out.get("hello").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn resolve_plugin_runs_via_default_host() {
        let mut host = DefaultHost::new();
        host.register_plugin(ResolvePlugin::new());

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
            .resolve(&"resolve".into())
            .unwrap()
            .run(
                &CapabilityRequest {
                    capability: "resolve".into(),
                    payload: serde_json::json!({}),
                },
                &mut ctx,
            )
            .unwrap();
        assert!(resp.0.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn default_host_runs_resolve() {
        let out = default_pass_host().run(serde_json::json!({})).unwrap();
        assert!(out.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false));
    }
}
