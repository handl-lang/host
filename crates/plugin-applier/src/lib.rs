use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    HostConfig, PlatformError, Plugin, PluginContext,
};
use handl_plugin_ext_kit::BasicKitLoader;

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
        let source = std::fs::read_to_string(&self.config_source)
            .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;
        let config = HostConfig::from_toml(&source)?;

        for kit in &config.kits {
            let plugin = BasicKitLoader::new().load(&kit.include)?;
            ctx.register_plugin(plugin)?;
        }

        if config.kits.is_empty() {
            if let Some(fallback) = &self.fallback_kit {
                let plugin = BasicKitLoader::new().load(fallback)?;
                ctx.register_plugin(plugin)?;
            }
        }

        for _plugin in &config.plugins {
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
