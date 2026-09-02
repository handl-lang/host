//! # handl-lib-hello
//!
//! Minimal example standard library for the handl runtime.
//!
//! Provides the `hello` library as a `Library` plugin.
//!
//! The greet *logic* is handl-authored in `kit-stdlib/data/hello/greet.hnl`;
//! this crate only loads + evaluates + bridges that definition (see
//! [`bridge`]). The `Module`/`Plugin` trait scaffolding is retained as the
//! registration boundary.

/// Handl-authored `greet` loader/bridge (shared by the `Function` and
/// `Plugin` impls).
mod bridge;
pub(crate) use bridge::greet_via_handl;

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
        let greeting = greet_via_handl(&name).map_err(|e| PlatformError::Plugin(e.to_string()))?;
        Ok(CapabilityResponse(serde_json::Value::String(greeting)))
    }
}

#[cfg(test)]
mod tests {
    use super::greet_via_handl;

    #[test]
    fn greet_via_handl_returns_canonical_greeting() {
        // The greet logic is handl-authored in greet.hnl; this confirms the
        // shared bridge loads + evaluates it.
        assert_eq!(greet_via_handl("world").unwrap(), "Hello, world!");
    }

    #[test]
    fn greet_via_handl_escapes_special_chars() {
        assert_eq!(greet_via_handl("a\"b\\c").unwrap(), "Hello, a\"b\\c!");
    }
}
