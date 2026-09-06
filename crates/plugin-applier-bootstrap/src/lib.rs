use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_applier::ApplierFactory;
use handl_plugin_applier_default::DefaultApplierFactory;

pub struct BootstrapPlugin {
    config_path: String,
    fallback_kit: String,
    applier_id: String,
    factories: Vec<Box<dyn ApplierFactory>>,
}

impl BootstrapPlugin {
    pub fn new(
        config_path: impl Into<String>,
        fallback_kit: impl Into<String>,
        applier_id: impl Into<String>,
    ) -> Self {
        Self {
            config_path: config_path.into(),
            fallback_kit: fallback_kit.into(),
            applier_id: applier_id.into(),
            factories: vec![Box::new(DefaultApplierFactory)],
        }
    }

    /// Register additional applier factories the bootstrap can select from.
    pub fn with_factories(
        mut self,
        factories: impl IntoIterator<Item = Box<dyn ApplierFactory>>,
    ) -> Self {
        self.factories.extend(factories);
        self
    }
}

impl Plugin for BootstrapPlugin {
    fn id(&self) -> &str {
        "bootstrap"
    }

    fn name(&self) -> &str {
        "bootstrap"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        match self.factories.iter().find(|f| f.id() == self.applier_id) {
            Some(factory) => {
                factory.register(ctx, &self.config_path, &self.fallback_kit)?;
            }
            None => {
                return Err(PlatformError::InvalidMetadata(format!(
                    "unknown applier: {}",
                    self.applier_id
                )));
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
