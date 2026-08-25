use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Kit, KitManifest, PlatformError, Plugin, PluginContext,
};

pub struct KitManagerPlugin {
    capabilities: Vec<CapabilityId>,
}

impl KitManagerPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("kit")],
        }
    }
}

impl Default for KitManagerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for KitManagerPlugin {
    fn name(&self) -> &str {
        "handl-ext-kit"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "kit",
                "description": "Manage kits"
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
                let plugins = request
                    .payload
                    .get("plugins")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<_>>()
                    })
                    .or_else(|| {
                        request.payload.get("args").and_then(|v| v.as_array()).map(|arr| {
                            arr.iter()
                                .skip(2)
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                    })
                    .unwrap_or_default();
                ctx.kits.register(Kit {
                    manifest: KitManifest {
                        id: name.into(),
                        name: name.into(),
                        plugins,
                        target: None,
                        selectors: Vec::new(),
                    },
                    available_plugins: Vec::new(),
                });
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
                ctx.kits.remove(name)?;
                Ok(CapabilityResponse(serde_json::json!({"removed": name})))
            }
            "info" => {
                let names: Vec<String> =
                    ctx.kits.all().iter().map(|k| k.manifest.name.clone()).collect();
                Ok(CapabilityResponse(serde_json::json!({"kits": names})))
            }
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
        let plugin = KitManagerPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("kit")).unwrap();
        assert_eq!(meta["command"], "kit");
        assert_eq!(meta["description"], "Manage kits");
    }

    #[test]
    fn add_registers_kit() {
        let plugin = KitManagerPlugin::new();
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
                    capability: "kit".into(),
                    payload: serde_json::json!({
                        "action": "add",
                        "name": "core",
                        "plugins": ["fmt", "lint"]
                    }),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"added": "core"}));
        assert!(ctx.kits.resolve("core").is_some());
    }

    #[test]
    fn info_lists_registered_kits() {
        let plugin = KitManagerPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        kits.register(Kit {
            manifest: KitManifest {
                id: "core".into(),
                name: "core".into(),
                plugins: vec!["fmt".into()],
                target: None,
                selectors: Vec::new(),
            },
            available_plugins: Vec::new(),
        });
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "kit".into(),
                    payload: serde_json::json!({"action": "info"}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["kits"], serde_json::json!(["core"]));
    }

    #[test]
    fn add_registers_kit_from_args() {
        let plugin = KitManagerPlugin::new();
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
                    capability: "kit".into(),
                    payload: serde_json::json!({"args": ["add", "my-kit", "fmt", "lint"]}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"added": "my-kit"}));
        let kit = ctx.kits.resolve("my-kit").unwrap();
        assert_eq!(kit.manifest.plugins, vec!["fmt".to_string(), "lint".to_string()]);
    }

    #[test]
    fn info_lists_kits_from_args() {
        let plugin = KitManagerPlugin::new();
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        kits.register(Kit {
            manifest: KitManifest {
                id: "core".into(),
                name: "core".into(),
                plugins: vec!["fmt".into()],
                target: None,
                selectors: Vec::new(),
            },
            available_plugins: Vec::new(),
        });
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "kit".into(),
                    payload: serde_json::json!({"args": ["info"]}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["kits"], serde_json::json!(["core"]));
    }

    #[test]
    fn invalid_action_rejected() {
        let plugin = KitManagerPlugin::new();
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
                    capability: "kit".into(),
                    payload: serde_json::json!({"action": "dance"}),
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::InvalidMetadata(_)));
    }
}
