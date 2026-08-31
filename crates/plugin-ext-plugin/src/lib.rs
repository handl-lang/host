use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext, ProcessPlugin,
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
        "handl-plugin-ext-plugin"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities.contains(capability).then(|| {
            serde_json::json!({
                "command": "plugin",
                "description": "Manage plugins"
            })
        })
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

                let path = request.payload.get("path").and_then(|v| v.as_str());
                let url = request.payload.get("url").and_then(|v| v.as_str());

                if path.is_some() || url.is_some() {
                    let manifest = serde_json::json!({
                        "id": name,
                        "version": "0.1.0",
                        "provides": capabilities.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                        "requires": [],
                        "optional": [],
                        "extends": [],
                        "conflicts": [],
                        "replaces": [],
                        "targets": [],
                        "exclusive": [],
                        "metadata": if let Some(p) = path {
                            serde_json::json!({"path": p})
                        } else {
                            serde_json::json!({"url": url})
                        },
                        "ast_kinds": [],
                    });

                    let load_request = CapabilityRequest {
                        capability: "load".into(),
                        payload: manifest,
                    };

                    let load_plugin = ctx
                        .plugins
                        .resolve(&"load".into())
                        .ok_or_else(|| PlatformError::UnknownCapability("load".into()))?;
                    load_plugin.run(&load_request, ctx)?;
                    Ok(CapabilityResponse(serde_json::json!({
                        "added": name,
                        "loaded": true
                    })))
                } else {
                    ctx.plugins
                        .register(ProcessPlugin::new(name, &capabilities));
                    Ok(CapabilityResponse(serde_json::json!({"added": name})))
                }
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
            _ => Err(PlatformError::InvalidMetadata(format!(
                "unknown action: {action}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{KitRegistry, PluginRegistry};

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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        plugins.register(ProcessPlugin::new("remote", &["compile".into()]));
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
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
    fn remove_removes_registered_plugin() {
        let plugin = PluginManagerPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        plugins.register(ProcessPlugin::new("remote", &[]));
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = plugin
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
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"removed": "remote"}));
        assert!(ctx.plugins.resolve(&"remote".into()).is_none());
    }

    #[test]
    fn add_registers_process_plugin_from_args() {
        let plugin = PluginManagerPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        plugins.register(ProcessPlugin::new("remote", &["compile".into()]));
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
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

    #[test]
    fn add_with_path_delegates_to_load() {
        use handl_platform::RuleAcceptor;
        use handl_plugin_load::{LoaderHostPlugin, LoaderRule, PluginLoader};

        struct MockLoader;
        impl PluginLoader for MockLoader {
            fn can_load(&self, manifest: &handl_platform::PluginManifest) -> bool {
                manifest.metadata.get("path").is_some()
            }
            fn load(
                &self,
                manifest: &handl_platform::PluginManifest,
            ) -> Result<Box<dyn Plugin + 'static>, PlatformError> {
                Ok(Box::new(ProcessPlugin::with_run_fn(
                    manifest.id.clone(),
                    &manifest.provides,
                    |req: &CapabilityRequest, _ctx: &mut PluginContext| {
                        Ok(CapabilityResponse(serde_json::json!({
                            "loaded": req.capability.as_str(),
                        })))
                    },
                )))
            }
        }

        let plugin = PluginManagerPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        plugins.register(LoaderHostPlugin::default());
        {
            let host = plugins
                .get_plugin_mut::<LoaderHostPlugin>()
                .expect("loader host registered");
            host.accept(LoaderRule {
                id: "mock".to_string(),
                loader: Box::new(MockLoader),
            })
            .unwrap();
        }
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "plugin".into(),
                    payload: serde_json::json!({
                        "action": "add",
                        "name": "remote",
                        "capabilities": ["echo"],
                        "path": "/some/path.so",
                    }),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            resp.0,
            serde_json::json!({"added": "remote", "loaded": true})
        );

        let loaded = ctx.plugins.resolve(&"echo".into()).unwrap();
        let mut run_ctx = PluginContext {
            plugins: &mut ctx.plugins,
            kits: &mut ctx.kits,
        };
        let r = loaded
            .run(
                &CapabilityRequest {
                    capability: "echo".into(),
                    payload: serde_json::Value::Null,
                },
                &mut run_ctx,
            )
            .unwrap();
        assert_eq!(r.0["loaded"], "echo");
    }

    #[test]
    fn add_with_path_without_loader_returns_unknown_capability() {
        let plugin = PluginManagerPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "plugin".into(),
                    payload: serde_json::json!({
                        "action": "add",
                        "name": "remote",
                        "capabilities": ["echo"],
                        "path": "/some/path.so",
                    }),
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
