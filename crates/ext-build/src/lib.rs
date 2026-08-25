use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin, PluginContext,
};
use handl_tooling::{CommandProvider, CommandSpec};

pub struct BuildPlugin {
    capabilities: Vec<CapabilityId>,
}

impl BuildPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("build")],
        }
    }
}

impl Default for BuildPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BuildPlugin {
    fn name(&self) -> &str {
        "handl-ext-build"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "build",
                "description": "Build a Handl project"
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
        Ok(CapabilityResponse(serde_json::json!({"built": true})))
    }
}

impl CommandProvider for BuildPlugin {
    fn commands(&self) -> Vec<CommandSpec> {
        vec![CommandSpec::new("build", "Build a Handl project")]
    }

    fn run(
        &self,
        _command: &str,
        _args: &[String],
        _host: &handl_tooling::ToolingHost,
    ) -> Result<serde_json::Value, PlatformError> {
        Ok(serde_json::json!({"built": true}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{ExtensionRegistry, KitRegistry, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = BuildPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("build")).unwrap();
        assert_eq!(meta["command"], "build");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_built_flag() {
        let plugin = BuildPlugin::new();
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
                    capability: "build".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"built": true}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BuildPlugin::new();
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
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
