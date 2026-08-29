//! # handl-lib-hello
//!
//! Minimal example standard library for the handl runtime.
//!
//! Provides the `hello` library as a `Library` plugin.

/// Greet `name` with the canonical Handl hello message.
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

fn parse_greet_arg(arg: &str) -> Option<String> {
    let rest = arg.strip_prefix("hello.greet")?;
    let rest = rest.trim();
    let inner = rest.strip_prefix("(")?.strip_suffix(")")?;
    let inner = inner.trim();
    let quoted = inner.strip_prefix('"')?.strip_suffix('"')?;
    Some(quoted.to_string())
}

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};

pub mod module;
pub use module::HelloModule;
pub mod plugin;
pub use plugin::HelloLibraryPlugin;

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
    fn id(&self) -> &'static str {
        "lib.hello.greet"
    }

    fn name(&self) -> &'static str {
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
        let name = if let Some(name) = request.payload.get("name").and_then(|v| v.as_str()) {
            name.to_string()
        } else if let Some(arg) = request
            .payload
            .get("args")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
        {
            parse_greet_arg(arg).unwrap_or_else(|| "world".to_string())
        } else {
            "world".to_string()
        };
        Ok(CapabilityResponse(serde_json::Value::String(greet(&name))))
    }
}
