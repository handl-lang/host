use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_applier_default::DefaultApplierPlugin;

pub struct BootstrapPlugin {
    config_path: String,
    fallback_kit: String,
    applier_id: String,
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
        }
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
        match self.applier_id.as_str() {
            "applier.default" => {
                ctx.register_plugin(
                    DefaultApplierPlugin::new(self.config_path.clone())
                        .with_fallback_kit(self.fallback_kit.clone()),
                )?;
            }
            other => {
                return Err(PlatformError::InvalidMetadata(format!(
                    "unknown applier: {other}"
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
mod tests {
    use super::*;
    use handl_platform::DefaultHost;
    use std::fs;

    #[test]
    fn bootstrap_loads_kit_via_applier() {
        let mut path = std::env::temp_dir();
        path.push("bootstrap_test.handl.toml");
        fs::write(&path, "[[kit]]\ninclude = \"hello\"\n").unwrap();

        let mut host = DefaultHost::new();
        host.register_plugin(BootstrapPlugin::new(
            path.as_path().to_str().unwrap(),
            "canonical",
            "applier.default",
        ));
        host.load().unwrap();

        let names = host.plugins().names();
        assert!(names.iter().any(|n| n == "lib-hello"), "names: {names:?}");
        assert!(
            names.iter().any(|n| n == "default-applier"),
            "names: {names:?}"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn bootstrap_rejects_unknown_applier() {
        let mut host = DefaultHost::new();
        host.register_plugin(BootstrapPlugin::new(
            ".handl.toml",
            "hello",
            "applier.custom",
        ));
        let result = host.load();
        assert!(result.is_err());
    }
}
