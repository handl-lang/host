use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_kit_data::{LibraryPlugin, LibraryRegistry};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    Kit, KitManifest, PlatformError, Plugin, PluginContext,
};

pub mod library_registry;

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
        "handl-plugin-ext-kit"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities.contains(capability).then(|| {
            serde_json::json!({
                "command": "kit",
                "description": "Manage kits"
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
                        request
                            .payload
                            .get("args")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
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
                let names: Vec<String> = ctx
                    .kits
                    .all()
                    .iter()
                    .map(|k| k.manifest.name.clone())
                    .collect();
                Ok(CapabilityResponse(serde_json::json!({"kits": names})))
            }
            _ => Err(PlatformError::InvalidMetadata(format!(
                "unknown action: {action}"
            ))),
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

/// A kit plugin assembled from a data manifest: the `module-runtime` host
/// plus an ordered set of library module plugins resolved from a
/// [`LibraryRegistry`].
///
/// `ModuleRuntime` is registered **concretely** (not boxed) because rule
/// contribution resolves acceptors by concrete type via
/// `get_plugin_mut::<ModuleRuntime>`; the remaining library modules are
/// registered as boxed plugins in manifest order, so the host's fixed-point
/// load calls their `contribute` exactly as the hand-written kits did.
pub struct ModuleKit {
    id: String,
    display_name: String,
    registers_module_runtime: bool,
    modules: Mutex<Vec<LibraryPlugin>>,
    contributed: AtomicBool,
}

impl ModuleKit {
    /// Create a `ModuleKit` for the kit named `name` from already-resolved
    /// `(module-name, plugin)` pairs in contribution order.
    pub fn new(name: &str, modules: Vec<(String, LibraryPlugin)>) -> Self {
        let registers_module_runtime = modules.iter().any(|(n, _)| n == "module-runtime");
        Self {
            id: format!("kit.{name}"),
            display_name: format!("kit-{name}"),
            registers_module_runtime,
            modules: Mutex::new(
                modules
                    .into_iter()
                    .filter(|(n, _)| n != "module-runtime")
                    .map(|(_, p)| p)
                    .collect(),
            ),
            contributed: AtomicBool::new(false),
        }
    }
}

impl Plugin for ModuleKit {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.display_name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.contributed.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.contributed.store(true, Ordering::SeqCst);
        // Register `ModuleRuntime` first so library plugins can contribute
        // their `ModuleRule`s into it (mirrors the built-in kits, which list
        // `module-runtime` first).
        if self.registers_module_runtime {
            ctx.register_plugin(handl_plugin_module::ModuleRuntime::default())?;
        }
        let modules = std::mem::take(
            &mut *self
                .modules
                .lock()
                .map_err(|_| PlatformError::Plugin("module kit lock poisoned".into()))?,
        );
        for plugin in modules {
            ctx.register_plugin(plugin)?;
        }
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

/// A concrete kit plugin assembled from data manifests.
pub enum BasicKit {
    /// A kit assembled from an embedded data manifest (`data/kits/*.yaml`).
    Modules(ModuleKit),
    /// A kit loaded from a manifest path.
    Manifest(ManifestKit),
}

impl Plugin for BasicKit {
    fn id(&self) -> &str {
        match self {
            Self::Modules(k) => k.id(),
            Self::Manifest(k) => k.id(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Modules(k) => k.name(),
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
            Self::Modules(k) => k.contribute(ctx),
            Self::Manifest(k) => k.contribute(ctx),
        }
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        match self {
            Self::Modules(k) => k.run(request, ctx),
            Self::Manifest(k) => k.run(request, ctx),
        }
    }
}

/// A plugin that exposes the built-in kit loader as a `kit/load` capability.
pub struct BasicKitLoaderPlugin {
    capabilities: Vec<CapabilityId>,
}

impl BasicKitLoaderPlugin {
    /// Creates a new `BasicKitLoaderPlugin`.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("kit/load")],
        }
    }

    fn load(&self, name_or_path: &str) -> Result<BasicKit, PlatformError> {
        if is_kit_path(name_or_path) {
            let manifest = KitManifest::load(name_or_path)?;
            return Ok(BasicKit::Manifest(ManifestKit::new(manifest)));
        }

        // Kit composition is data: load the embedded manifest for the kit
        // name (`handl-kit-data-canonical/data/kits/<name>.yaml`), then
        // resolve each listed library module through the populated
        // `LibraryRegistry`.
        let module_names = handl_kit_data_canonical::kit_module_names(name_or_path)
            .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;
        let mut registry = LibraryRegistry::new();
        library_registry::populate(&mut registry);
        let mut modules = Vec::with_capacity(module_names.len());
        for name in module_names {
            let plugin = registry.resolve(&name).ok_or_else(|| {
                PlatformError::InvalidMetadata(format!("unknown library plugin: {name}"))
            })?;
            modules.push((name, plugin));
        }
        Ok(BasicKit::Modules(ModuleKit::new(name_or_path, modules)))
    }
}

impl Default for BasicKitLoaderPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicKitLoaderPlugin {
    fn id(&self) -> &str {
        "kit.loader.basic"
    }

    fn name(&self) -> &str {
        "basic-kit-loader"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        Ok(())
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "kit/load" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        let name = serde_json::from_value::<String>(request.payload.clone())
            .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;
        let kit = self.load(&name)?;
        ctx.plugins.register(kit);
        Ok(CapabilityResponse(serde_json::Value::Null))
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

    /// Every module name listed in the embedded kit manifests must resolve
    /// through the populated [`LibraryRegistry`] — including the
    /// table-driven libraries (`utils`, `cmp`, `json`, `toml`, `yaml`,
    /// `stats`) now built from `handl-plugin-lib-stdlib`'s package table.
    #[test]
    fn builtin_kit_module_names_resolve() {
        let loader = BasicKitLoaderPlugin::new();
        for kit in ["hello", "stdlib", "cli", "config", "server"] {
            loader
                .load(kit)
                .unwrap_or_else(|e| panic!("kit '{kit}' failed to load: {e}"));
        }
        // The table-driven packages resolve to PackagePlugins with the
        // same ids/names as the deleted per-crate shells.
        let mut registry = LibraryRegistry::new();
        library_registry::populate(&mut registry);
        let utils = registry.resolve("utils").expect("utils plugin");
        assert_eq!(utils.id(), "lib.utils");
        assert_eq!(utils.name(), "lib-utils");
        let cmp = registry.resolve("cmp").expect("cmp plugin");
        assert_eq!(cmp.id(), "lib.cmp");
        let stats = registry.resolve("stats").expect("stats plugin");
        assert_eq!(stats.id(), "lib.stats");
    }

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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
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
        assert_eq!(
            kit.manifest.plugins,
            vec!["fmt".to_string(), "lint".to_string()]
        );
    }

    #[test]
    fn info_lists_kits_from_args() {
        let plugin = KitManagerPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
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
