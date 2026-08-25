//! # handl-compiler-pass-generate
//!
//! `GeneratePass` wraps the JSON payload in a `{"generated": ...}` object
//! and can be registered as a `handl-platform` `Plugin`.

use handl_compiler::{CompilerError, Pass, PassHost};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

mod stage;
pub use stage::*;

/// Generate pass: wraps the payload as `{"generated": input}`.
pub struct GeneratePass;

impl GeneratePass {
    pub const fn new() -> Self {
        Self
    }
}

impl Pass for GeneratePass {
    fn id(&self) -> &str {
        "generate"
    }

    fn run(&self, input: Value) -> Result<Value, CompilerError> {
        Ok(serde_json::json!({
            "generated": input,
            "stage": self.id(),
        }))
    }
}

/// Plugin wrapper for `GeneratePass`.
pub struct GeneratePlugin {
    pass: GeneratePass,
    capabilities: Vec<CapabilityId>,
}

impl GeneratePlugin {
    pub fn new() -> Self {
        Self {
            pass: GeneratePass::new(),
            capabilities: vec!["generate".into()],
        }
    }
}

impl Default for GeneratePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for GeneratePlugin {
    fn name(&self) -> &str {
        "handl-compiler-pass-generate"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({"command": "generate", "description": "Generate output from the typechecked document"}))
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

/// Default `PassHost` pre-registered with `GeneratePass`.
pub fn default_pass_host() -> PassHost {
    let mut host = PassHost::new();
    host.register(GeneratePass::new());
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{DefaultHost, Host, PluginRegistry};

    #[test]
    fn generate_pass_wraps_input() {
        let out = GeneratePass::new()
            .run(serde_json::json!({"hello": true}))
            .unwrap();
        let generated = out.get("generated").unwrap();
        assert!(generated.get("hello").and_then(|v| v.as_bool()).unwrap_or(false));
        assert_eq!(out.get("stage").and_then(|v| v.as_str()), Some("generate"));
    }

    #[test]
    fn generate_plugin_runs_via_default_host() {
        let mut host = DefaultHost::new();
        host.register_plugin(GeneratePlugin::new());

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
            .resolve(&"generate".into())
            .unwrap()
            .run(
                &CapabilityRequest {
                    capability: "generate".into(),
                    payload: serde_json::json!({}),
                },
                &mut ctx,
            )
            .unwrap();
        assert!(resp.0.get("generated").is_some());
    }

    #[test]
    fn default_host_runs_generate() {
        let out = default_pass_host().run(serde_json::json!({})).unwrap();
        assert!(out.get("generated").is_some());
    }
}
