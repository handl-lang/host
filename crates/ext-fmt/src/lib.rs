use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_tooling::{ArgSpec, CommandProvider, CommandSpec, read_source_or_stdin};

pub struct FmtPlugin {
    capabilities: Vec<CapabilityId>,
}

impl FmtPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("fmt")],
        }
    }
}

impl Default for FmtPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for FmtPlugin {
    fn name(&self) -> &str {
        "handl-ext-fmt"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "fmt",
                "description": "Format a Handl module"
            }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if !self.capabilities.contains(&request.capability) {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        Ok(CapabilityResponse(serde_json::json!({"formatted": true})))
    }
}

impl CommandProvider for FmtPlugin {
    fn commands(&self) -> Vec<CommandSpec> {
        vec![CommandSpec::new("fmt", "Format a Handl source file")
            .arg(ArgSpec::new("file").description("File to format; stdin if omitted"))]
    }

    fn run(
        &self,
        _command: &str,
        args: &[String],
        host: &handl_tooling::ToolingHost,
    ) -> Result<serde_json::Value, PlatformError> {
        let source = read_source_or_stdin(args.first().map(|s| s.as_str()))?;
        let formatted = host.format(&source)?;
        Ok(serde_json::Value::String(formatted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{ExtensionRegistry, KitRegistry, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = FmtPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("fmt")).unwrap();
        assert_eq!(meta["command"], "fmt");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_fmted_flag() {
        let plugin = FmtPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"formatted": true}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = FmtPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
