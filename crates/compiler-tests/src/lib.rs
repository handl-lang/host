//! Shared test helpers and mocks for the `handl-compiler` integration tests.

use handl_compiler_common::{CompilerError, Pass};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, DefaultHost,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// `Pass` that appends a `marker: true` key to an object payload.
pub struct MockPass {
    marker: &'static str,
    fail: bool,
}

impl MockPass {
    pub const fn new(marker: &'static str) -> Self {
        Self {
            marker,
            fail: false,
        }
    }

    pub const fn failing(marker: &'static str) -> Self {
        Self {
            marker,
            fail: true,
        }
    }
}

impl Pass for MockPass {
    fn id(&self) -> &str {
        self.marker
    }

    fn run(&self, input: Value) -> Result<Value, CompilerError> {
        if self.fail {
            return Err(CompilerError::PassFailed(self.marker.to_string()));
        }
        let mut obj = match input {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        obj.insert(self.marker.to_string(), Value::Bool(true));
        Ok(Value::Object(obj))
    }
}

/// `Plugin` wrapper around a `MockPass`.
pub struct MockPlugin {
    pass: MockPass,
    capabilities: Vec<CapabilityId>,
}

impl MockPlugin {
    pub fn new(marker: &'static str) -> Self {
        Self {
            pass: MockPass::new(marker),
            capabilities: vec![marker.into()],
        }
    }

    pub fn failing(marker: &'static str) -> Self {
        Self {
            pass: MockPass::failing(marker),
            capabilities: vec![marker.into()],
        }
    }
}

impl Plugin for MockPlugin {
    fn name(&self) -> &str {
        self.pass.id()
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({"command": self.pass.id(), "description": "mock"}))
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

/// Build a `DefaultHost` with a mock `Plugin` for a single stage.
pub fn mock_host(stage: &'static str) -> DefaultHost {
    let mut host = DefaultHost::new();
    host.register_plugin(MockPlugin::new(stage));
    host
}

/// Build a `DefaultHost` with all five default `handl-compiler-pass-*`
/// plugins registered.
pub fn full_pass_host() -> DefaultHost {
    let mut host = DefaultHost::new();
    host.register_plugin(handl_compiler_pass_parse::ParsePlugin::new());
    host.register_plugin(handl_compiler_pass_extract::ExtractPlugin::new());
    host.register_plugin(handl_compiler_pass_resolve::ResolvePlugin::new());
    host.register_plugin(handl_compiler_pass_typecheck::TypecheckPlugin::new());
    host.register_plugin(handl_compiler_pass_generate::GeneratePlugin::new());
    host
}
