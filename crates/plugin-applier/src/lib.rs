use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    HostConfig, PlatformError, Plugin, PluginContext,
};
use handl_plugin_ext_kit::BasicKitLoaderPlugin;

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
