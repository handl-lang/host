use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    HostConfig, OptionsPlugin, PlatformError, Plugin, PluginContext, PluginManifest,
};
use handl_plugin_applier::ApplierFactory;
use handl_kit_data::LibraryRegistry;
use handl_plugin_ext_kit::{BasicKitLoaderPlugin, KitModuleNames};
use std::sync::Arc;
use handl_plugin_load::LoaderHostPlugin;

/// Factory for the default applier, selected by `--applier applier.default`.
///
/// Carries the host-injected kit-manifest resolver (ADR-0128 §"The 0.8
/// seam"): the applier registers a [`BasicKitLoaderPlugin`], which needs a
/// kit-name → module-names lookup it cannot source itself (the embedded
/// manifests live in `handl-kit-data-canonical`, a `lang/` crate).
pub struct DefaultApplierFactory {
    module_names: KitModuleNames,
    libraries: Arc<LibraryRegistry>,
}

impl DefaultApplierFactory {
    /// Create the factory with the host's kit-manifest resolver and the
    /// kit's library-module registry (ADR-0128 §0.4).
    pub fn new(module_names: KitModuleNames, libraries: Arc<LibraryRegistry>) -> Self {
        Self {
            module_names,
            libraries,
        }
    }
}

impl ApplierFactory for DefaultApplierFactory {
    fn id(&self) -> &str {
        "applier.default"
    }

    fn register(
        &self,
        ctx: &mut ContributionContext,
        config_path: &str,
        fallback_kit: &str,
    ) -> Result<(), PlatformError> {
        ctx.register_plugin(
            DefaultApplierPlugin::new(
                config_path,
                self.module_names.clone(),
                self.libraries.clone(),
            )
            .with_fallback_kit(fallback_kit),
        )
    }
}

pub struct DefaultApplierPlugin {
    config_source: String,
    fallback_kit: Option<String>,
    /// Host-injected kit-name → module-names resolver for the registered
    /// [`BasicKitLoaderPlugin`].
    module_names: KitModuleNames,
    /// The kit-supplied library-module registry for the loader.
    libraries: Arc<LibraryRegistry>,
}

impl DefaultApplierPlugin {
    pub fn new(
        config_source: impl Into<String>,
        module_names: KitModuleNames,
        libraries: Arc<LibraryRegistry>,
    ) -> Self {
        Self {
            config_source: config_source.into(),
            fallback_kit: None,
            module_names,
            libraries,
        }
    }

    pub fn with_fallback_kit(mut self, kit: impl Into<String>) -> Self {
        self.fallback_kit = Some(kit.into());
        self
    }
}

impl Plugin for DefaultApplierPlugin {
    fn id(&self) -> &str {
        "applier.default"
    }

    fn name(&self) -> &str {
        "default-applier"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let config = match std::fs::read_to_string(&self.config_source) {
            Ok(source) => HostConfig::from_toml(&source)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HostConfig::new(),
            Err(e) => return Err(PlatformError::InvalidMetadata(e.to_string())),
        };

        if ctx.get_plugin_mut::<BasicKitLoaderPlugin>().is_none() {
            ctx.register_plugin(BasicKitLoaderPlugin::new(
                self.module_names.clone(),
                self.libraries.clone(),
            ))?;
        }

        if ctx.get_plugin_mut::<LoaderHostPlugin>().is_none() {
            ctx.register_plugin(LoaderHostPlugin::default())?;
        }

        if ctx.get_plugin_mut::<OptionsPlugin>().is_none() {
            ctx.register_plugin(OptionsPlugin::new())?;
        }

        for kit in &config.kits {
            let name = kit.path.as_ref().unwrap_or(&kit.include);
            ctx.run(&CapabilityRequest {
                capability: "kit/load".into(),
                payload: serde_json::Value::String(name.clone()),
            })?;
        }

        if config.kits.is_empty() {
            if let Some(fallback) = &self.fallback_kit {
                ctx.run(&CapabilityRequest {
                    capability: "kit/load".into(),
                    payload: serde_json::Value::String(fallback.clone()),
                })?;
            }
        }

        for plugin in &config.plugins {
            if plugin.register.is_empty() {
                if let Some(path) = &plugin.manifest {
                    let source = std::fs::read_to_string(path)
                        .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;
                    let manifest: PluginManifest = serde_json::from_str(&source)
                        .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;
                    ctx.run(&CapabilityRequest {
                        capability: "load".into(),
                        payload: serde_json::to_value(&manifest)
                            .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?,
                    })?;
                }
            }
        }

        for (id, value) in &config.options {
            ctx.run(&CapabilityRequest {
                capability: "option/set".into(),
                payload: serde_json::json!({ "id": id, "value": value }),
            })?;
        }

        for id in &config.excludes {
            ctx.exclude_plugin(id.clone());
        }

        for kit in &config.kits {
            for id in &kit.exclude {
                ctx.exclude_plugin(id.clone());
            }
        }

        Ok(())
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::UnknownCapability(request.capability.clone()))
    }
}

#[cfg(test)]
mod lib_tests;
