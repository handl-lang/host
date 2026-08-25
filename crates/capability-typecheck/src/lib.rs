use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin, PluginContext,
};

pub struct TypecheckPlugin {
    capabilities: Vec<CapabilityId>,
}

impl TypecheckPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("typecheck")],
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
        "handl-capability-typecheck"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "typecheck",
                "description": "Typecheck a Handl module"
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
        Ok(CapabilityResponse(serde_json::json!({"typechecked": true})))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{ExtensionRegistry, KitRegistry, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = TypecheckPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("typecheck")).unwrap();
        assert_eq!(meta["command"], "typecheck");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_typechecked_flag() {
        let plugin = TypecheckPlugin::new();
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
                    capability: "typecheck".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = TypecheckPlugin::new();
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
