use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, Extension, PlatformError, Plugin, PluginContext,
};
use handl_tooling::{ArgSpec, CommandProvider, CommandSpec};

pub struct SimpleExtension {
    id: String,
    metadata: serde_json::Value,
}

impl SimpleExtension {
    pub fn new(id: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            metadata,
        }
    }
}

impl Extension for SimpleExtension {
    fn id(&self) -> &str {
        &self.id
    }

    fn metadata(&self) -> serde_json::Value {
        self.metadata.clone()
    }
}

pub struct ExtensionManagerPlugin {
    capabilities: Vec<CapabilityId>,
}

impl ExtensionManagerPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("extension")],
        }
    }
}

impl Default for ExtensionManagerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ExtensionManagerPlugin {
    fn name(&self) -> &str {
        "handl-ext-extension"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "extension",
                "description": "Manage extensions"
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
                let metadata = request
                    .payload
                    .get("metadata")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                ctx.extensions.register(SimpleExtension::new(name, metadata));
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
                ctx.extensions.remove(name)?;
                Ok(CapabilityResponse(serde_json::json!({"removed": name})))
            }
            "info" => {
                let ids: Vec<String> =
                    ctx.extensions.all().iter().map(|e| e.id().to_string()).collect();
                Ok(CapabilityResponse(serde_json::json!({"extensions": ids})))
            }
            _ => Err(PlatformError::InvalidMetadata(format!("unknown action: {action}"))),
        }
    }
}

impl CommandProvider for ExtensionManagerPlugin {
    fn commands(&self) -> Vec<CommandSpec> {
        vec![CommandSpec::new("extension", "Manage extensions")
            .arg(ArgSpec::new("args").description("Extension action and arguments").multiple())]
    }

    fn run(
        &self,
        _command: &str,
        _args: &[String],
        _host: &handl_tooling::ToolingHost,
    ) -> Result<serde_json::Value, PlatformError> {
        Ok(serde_json::json!({"extensions": []}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{ExtensionRegistry, KitRegistry, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = ExtensionManagerPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("extension")).unwrap();
        assert_eq!(meta["command"], "extension");
        assert_eq!(meta["description"], "Manage extensions");
    }

    #[test]
    fn add_registers_extension() {
        let plugin = ExtensionManagerPlugin::new();
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
                    capability: "extension".into(),
                    payload: serde_json::json!({
                        "action": "add",
                        "name": "foo",
                        "metadata": {"name": "foo"}
                    }),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"added": "foo"}));
        assert!(ctx.extensions.get("foo").is_some());
    }

    #[test]
    fn add_registers_extension_from_args() {
        let plugin = ExtensionManagerPlugin::new();
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
                    capability: "extension".into(),
                    payload: serde_json::json!({"args": ["add", "foo"]}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"added": "foo"}));
        assert!(ctx.extensions.get("foo").is_some());
    }

    #[test]
    fn info_lists_registered_extensions() {
        let plugin = ExtensionManagerPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        extensions.register(SimpleExtension::new("foo", serde_json::json!({"name":"foo"})));
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "extension".into(),
                    payload: serde_json::json!({"action": "info"}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["extensions"], serde_json::json!(["foo"]));
    }

    #[test]
    fn info_lists_extensions_from_args() {
        let plugin = ExtensionManagerPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        extensions.register(SimpleExtension::new("foo", serde_json::json!({"name":"foo"})));
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "extension".into(),
                    payload: serde_json::json!({"args": ["info"]}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["extensions"], serde_json::json!(["foo"]));
    }

    #[test]
    fn invalid_action_rejected() {
        let plugin = ExtensionManagerPlugin::new();
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
                    capability: "extension".into(),
                    payload: serde_json::json!({"action": "dance"}),
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::InvalidMetadata(_)));
    }
}
