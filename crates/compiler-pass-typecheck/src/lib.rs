//! # handl-compiler-pass-typecheck
//!
//! `TypecheckPass` adds a deterministic `typechecked` marker to the JSON
//! payload and can be registered as a `handl-platform` `Plugin`.

use handl_compiler::{CompilerError, Pass, PassHost};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use serde_json::Value;

mod stage;
pub use stage::*;

/// Typecheck pass: marks the payload with `{"typechecked": true}`.
pub struct TypecheckPass;

impl TypecheckPass {
    pub const fn new() -> Self {
        Self
    }
}

impl Pass for TypecheckPass {
    fn id(&self) -> &str {
        "typecheck"
    }

    fn run(&self, input: Value) -> Result<Value, CompilerError> {
        let mut obj = match input {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        obj.insert("typechecked".to_string(), Value::Bool(true));
        obj.insert("stage".to_string(), Value::String(self.id().to_string()));
        Ok(Value::Object(obj))
    }
}

/// Plugin wrapper for `TypecheckPass`.
pub struct TypecheckPlugin {
    pass: TypecheckPass,
    capabilities: Vec<CapabilityId>,
}

impl TypecheckPlugin {
    pub fn new() -> Self {
        Self {
            pass: TypecheckPass::new(),
            capabilities: vec!["typecheck".into()],
        }
    }
}

impl Default for TypecheckPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TypecheckPlugin {
    fn name(&self) -> &str {
        "handl-compiler-pass-typecheck"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({"command": "typecheck", "description": "Typecheck the resolved document"}))
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

/// Default `PassHost` pre-registered with `TypecheckPass`.
pub fn default_pass_host() -> PassHost {
    let mut host = PassHost::new();
    host.register(TypecheckPass::new());
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{DefaultHost, Host, PluginRegistry};

    #[test]
    fn typecheck_pass_adds_typechecked_marker() {
        let out = TypecheckPass::new()
            .run(serde_json::json!({"hello": true}))
            .unwrap();
        assert!(out.get("hello").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("typechecked").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn typecheck_plugin_runs_via_default_host() {
        let mut host = DefaultHost::new();
        host.register_plugin(TypecheckPlugin::new());

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
            .resolve(&"typecheck".into())
            .unwrap()
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload: serde_json::json!({}),
                },
                &mut ctx,
            )
            .unwrap();
        assert!(resp.0.get("typechecked").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[test]
    fn default_host_runs_typecheck() {
        let out = default_pass_host().run(serde_json::json!({})).unwrap();
        assert!(out.get("typechecked").and_then(|v| v.as_bool()).unwrap_or(false));
    }
}
