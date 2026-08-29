use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    Kit, KitLoader, KitManifest, PlatformError, Plugin, PluginContext,
};
use handl_kit_canonical::CanonicalKit;
use handl_kit_hello::HelloKit;

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


/// A kit plugin backed by a loaded `KitManifest`.
pub struct ManifestKit {
    manifest: KitManifest,
}

impl ManifestKit {
    /// Create a new `ManifestKit` from a manifest.
    pub fn new(manifest: KitManifest) -> Self {
        Self { manifest }
    }
}

impl Default for ManifestKit {
    fn default() -> Self {
        Self {
            manifest: KitManifest::default(),
        }
    }
}

impl Plugin for ManifestKit {
    fn id(&self) -> &str {
        "kit.manifest"
    }

    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// A concrete kit plugin that wraps the built-in canonical and hello kits.
pub enum BasicKit {
    /// The canonical kit.
    Canonical(CanonicalKit),
    /// The hello kit.
    Hello(HelloKit),
    /// A kit loaded from a manifest path.
    Manifest(ManifestKit),
}

impl Default for BasicKit {
    fn default() -> Self {
        Self::Canonical(CanonicalKit::new())
    }
}

impl Plugin for BasicKit {
    fn id(&self) -> &str {
        match self {
            Self::Canonical(k) => k.id(),
            Self::Hello(k) => k.id(),
            Self::Manifest(k) => k.id(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Canonical(k) => k.name(),
            Self::Hello(k) => k.name(),
            Self::Manifest(k) => k.name(),
        }
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        match self {
            Self::Canonical(k) => k.contribute(ctx),
            Self::Hello(k) => k.contribute(ctx),
            Self::Manifest(k) => k.contribute(ctx),
        }
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        match self {
            Self::Canonical(k) => k.run(request, ctx),
            Self::Hello(k) => k.run(request, ctx),
            Self::Manifest(k) => k.run(request, ctx),
        }
    }
}

/// A static kit loader that maps built-in kit names to their plugins and
/// supports loading a `KitManifest` from a file path.
pub struct BasicKitLoader;

impl Default for BasicKitLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicKitLoader {
    /// Create a new `BasicKitLoader`.
    pub fn new() -> Self {
        Self
    }
}

impl KitLoader for BasicKitLoader {
    type Kit = BasicKit;

    fn load(&self, name_or_path: &str) -> Result<BasicKit, PlatformError> {
        if is_kit_path(name_or_path) {
            let manifest = KitManifest::load(name_or_path)?;
            return Ok(BasicKit::Manifest(ManifestKit::new(manifest)));
        }

        match name_or_path {
            "canonical" => Ok(BasicKit::Canonical(CanonicalKit::new())),
            "hello" => Ok(BasicKit::Hello(HelloKit::new())),
            _ => Err(PlatformError::InvalidMetadata(format!(
                "unknown kit: {name_or_path}"
            ))),
        }
    }
}

fn is_kit_path(s: &str) -> bool {
    let path = std::path::Path::new(s);
    path.exists() || s.ends_with(".toml") || s.ends_with(".json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{KitRegistry, PluginRegistry};

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
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
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
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
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
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
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
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
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
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
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
