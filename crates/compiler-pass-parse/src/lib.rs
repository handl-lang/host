//! # handl-compiler-pass-parse
//!
//! `ParsePass` adds a deterministic `parsed` marker to the JSON payload
//! and can be registered as a `handl-platform` `Plugin`.

use handl_compiler::{CompilerError, Pass, PassHost};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

mod stage;
pub use stage::*;

/// Parse pass: marks the payload with `{"parsed": true}`.
pub struct ParsePass;

impl ParsePass {
    pub const fn new() -> Self {
        Self
    }
}

impl Pass for ParsePass {
    fn id(&self) -> &str {
        "parse"
    }

    fn run(&self, input: Value) -> Result<Value, CompilerError> {
        let mut obj = match input {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        obj.insert("parsed".to_string(), Value::Bool(true));
        obj.insert("stage".to_string(), Value::String(self.id().to_string()));
        Ok(Value::Object(obj))
    }
}

/// Plugin wrapper for `ParsePass`.
pub struct ParsePlugin {
    pass: ParsePass,
    capabilities: Vec<CapabilityId>,
}

impl ParsePlugin {
    pub fn new() -> Self {
        Self {
            pass: ParsePass::new(),
            capabilities: vec!["parse".into()],
        }
    }
}

impl Default for ParsePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ParsePlugin {
    fn name(&self) -> &str {
        "handl-compiler-pass-parse"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({"command": "parse", "description": "Parse input into JSON"}))
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

/// Default `PassHost` pre-registered with `ParsePass`.
pub fn default_pass_host() -> PassHost {
    let mut host = PassHost::new();
    host.register(ParsePass::new());
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{DefaultHost, Host, PluginRegistry};

    #[test]
    fn parse_pass_adds_parsed_marker() {
        let out = ParsePass::new()
            .run(serde_json::json!({"hello": true}))
            .unwrap();
        assert!(out.get("hello").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("parsed").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn parse_plugin_runs_via_default_host() {
        let mut host = DefaultHost::new();
        host.register_plugin(ParsePlugin::new());

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
            .resolve(&"parse".into())
            .unwrap()
            .run(
                &CapabilityRequest {
                    capability: "parse".into(),
                    payload: serde_json::json!({}),
                },
                &mut ctx,
            )
            .unwrap();
        assert!(resp.0.get("parsed").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn default_host_runs_parse() {
        let out = default_pass_host().run(serde_json::json!({})).unwrap();
        assert!(out.get("parsed").and_then(|v| v.as_bool()).unwrap_or(false));
    }
}
