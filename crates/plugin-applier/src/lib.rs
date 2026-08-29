use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    HostConfig, PlatformError, Plugin, PluginContext, PluginManifest,
};
use handl_plugin_ext_kit::BasicKitLoaderPlugin;
use handl_plugin_load::LoaderHostPlugin;

pub struct DefaultApplierPlugin {
    config_source: String,
    fallback_kit: Option<String>,
}

impl DefaultApplierPlugin {
    pub fn new(config_source: impl Into<String>) -> Self {
        Self {
            config_source: config_source.into(),
            fallback_kit: None,
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
            ctx.register_plugin(BasicKitLoaderPlugin::new())?;
        }

        if ctx.get_plugin_mut::<LoaderHostPlugin>().is_none() {
            ctx.register_plugin(LoaderHostPlugin::default())?;
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
mod tests {
    use super::*;
    use handl_platform::{DefaultHost, Host};
    use handl_plugin_load::LoaderHostPlugin;
    use std::fs;

    #[test]
    fn applier_loads_kit_from_config() {
        let mut path = std::env::temp_dir();
        path.push("applier_test.handl.toml");
        fs::write(&path, "[[kit]]\ninclude = \"hello\"\n").unwrap();

        let mut host = DefaultHost::new();
        host.register_plugin(BasicKitLoaderPlugin::new());
        host.register_plugin(LoaderHostPlugin::default());
        host.register_plugin(
            DefaultApplierPlugin::new(path.as_path().to_str().unwrap()).with_fallback_kit("canonical"),
        );
        host.load().unwrap();

        let names = host.plugins().names();
        assert!(names.iter().any(|n| n == "hello"), "names: {names:?}");

        let _ = fs::remove_file(&path);
    }
}
