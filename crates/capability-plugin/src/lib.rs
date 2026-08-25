use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin, PluginContext, ProcessPlugin,
};

pub struct PluginManagerPlugin {
    capabilities: Vec<CapabilityId>,
}

impl PluginManagerPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("plugin")],
        }
    }
}

impl Default for PluginManagerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PluginManagerPlugin {
    fn name(&self) -> &str {
        "handl-capability-plugin"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "plugin",
                "description": "Manage plugins"
            }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if !self.capabilities.contains(&request.capability) {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let action = request
            .payload
            .get("action")
            .and_then(|v| v.as_str())
            .or_else(|| {
                request
                    .payload
                    .get("args")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
            })
            .ok_or_else(|| PlatformError::InvalidMetadata("missing action".into()))?;

        match action {
            "add" => {
                let name = request
                    .payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        request
                            .payload
                            .get("args")
                            .and_then(|v| v.as_array())
                            .and_then(|a| a.get(1))
                            .and_then(|v| v.as_str())
                    })
                    .ok_or_else(|| PlatformError::InvalidMetadata("missing name".into()))?;
                let capabilities = request
                    .payload
                    .get("capabilities")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(CapabilityId::from))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                ctx.plugins
                    .register(ProcessPlugin::new(name, &capabilities));
                Ok(CapabilityResponse(serde_json::json!({"added": name})))
            }
            "remove" => {
                let name = request
                    .payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        request
                            .payload
                            .get("args")
                            .and_then(|v| v.as_array())
                            .and_then(|a| a.get(1))
                            .and_then(|v| v.as_str())
                    })
                    .ok_or_else(|| PlatformError::InvalidMetadata("missing name".into()))?;
                ctx.plugins.remove(name)?;
                Ok(CapabilityResponse(serde_json::json!({"removed": name})))
            }
            "info" => Ok(CapabilityResponse(serde_json::json!({
                "plugins": ctx.plugins.names()
            }))),
            _ => Err(PlatformError::InvalidMetadata(format!("unknown action: {action}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{ExtensionRegistry, KitRegistry, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = PluginManagerPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("plugin")).unwrap();
        assert_eq!(meta["command"], "plugin");
        assert_eq!(meta["description"], "Manage plugins");
    }

    #[test]
    fn add_registers_process_plugin() {
        let plugin = PluginManagerPlugin::new();
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
                    capability: "plugin".into(),
                    payload: serde_json::json!({
                        "action": "add",
                        "name": "remote",
                        "capabilities": ["compile"]
                    }),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"added": "remote"}));
        assert!(ctx.plugins.resolve(&"compile".into()).is_some());
    }

    #[test]
    fn info_lists_registered_plugins() {
        let plugin = PluginManagerPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        plugins.register(ProcessPlugin::new("remote", &["compile".into()]));
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "plugin".into(),
                    payload: serde_json::json!({"action": "info"}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["plugins"], serde_json::json!(["remote"]));
    }

    #[test]
    fn remove_returns_not_implemented() {
        let plugin = PluginManagerPlugin::new();
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
                    capability: "plugin".into(),
                    payload: serde_json::json!({
                        "action": "remove",
                        "name": "remote"
                    }),
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::NotImplemented));
    }

    #[test]
    fn add_registers_process_plugin_from_args() {
        let plugin = PluginManagerPlugin::new();
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
                    capability: "plugin".into(),
                    payload: serde_json::json!({"args": ["add", "remote"]}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"added": "remote"}));
    }

    #[test]
    fn info_lists_plugins_from_args() {
        let plugin = PluginManagerPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        plugins.register(ProcessPlugin::new("remote", &["compile".into()]));
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "plugin".into(),
                    payload: serde_json::json!({"args": ["info"]}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["plugins"], serde_json::json!(["remote"]));
    }

    #[test]
    fn invalid_action_rejected() {
        let plugin = PluginManagerPlugin::new();
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
                    capability: "plugin".into(),
                    payload: serde_json::json!({"action": "dance"}),
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::InvalidMetadata(_)));
    }
}
