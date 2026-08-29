//! Abstract loader plugin mechanism.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, Contribution,
    Operation, PlatformError, Plugin, PluginContext, PluginManifest, RuleAcceptor,
};

/// A plugin loader that can load a plugin from a manifest.
pub trait PluginLoader: Send + Sync {
    /// Returns `true` if this loader can load the given manifest.
    fn can_load(&self, manifest: &PluginManifest) -> bool;
    /// Loads the plugin described by `manifest`.
    fn load(&self, manifest: &PluginManifest) -> Result<Box<dyn Plugin + 'static>, PlatformError>;
}

/// Marker type for the `Load` operation.
pub struct Load;

impl Operation for Load {
    type Request = PluginManifest;
}

/// A rule contribution that binds a loader to an identifier.
pub struct LoaderRule {
    /// Stable identifier for this loader rule.
    pub id: String,
    /// The loader implementation.
    pub loader: Box<dyn PluginLoader>,
}

impl Contribution for LoaderRule {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Host plugin that collects [`LoaderRule`] contributions and exposes a `load` capability.
pub struct LoaderHostPlugin {
    loaders: Vec<Box<dyn PluginLoader>>,
    capabilities: Vec<CapabilityId>,
}

impl Default for LoaderHostPlugin {
    fn default() -> Self {
        Self {
            loaders: Vec::new(),
            capabilities: vec![CapabilityId::from("load")],
        }
    }
}

impl Plugin for LoaderHostPlugin {
    fn id(&self) -> &str {
        "loader.host"
    }

    fn name(&self) -> &str {
        "loader-host"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, _ctx: &mut handl_platform::ContributionContext) -> Result<(), PlatformError> {
        Ok(())
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let manifest: PluginManifest = serde_json::from_value(request.payload.clone())
            .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;

        for loader in &self.loaders {
            if !loader.can_load(&manifest) {
                continue;
            }
            let plugin = loader.load(&manifest)?;
            ctx.plugins.register(LoadedPlugin(plugin));
            return Ok(CapabilityResponse(serde_json::Value::Null));
        }

        Err(PlatformError::Plugin("no loader could load manifest".to_string()))
    }
}

impl RuleAcceptor for LoaderHostPlugin {
    type Rule = LoaderRule;

    fn accept(&mut self, rule: LoaderRule) -> Result<(), PlatformError> {
        self.loaders.push(rule.loader);
        Ok(())
    }
}

/// Wrapper around a loaded plugin.
pub struct LoadedPlugin(pub Box<dyn Plugin + 'static>);

impl Plugin for LoadedPlugin {
    fn id(&self) -> &str {
        self.0.id()
    }

    fn name(&self) -> &str {
        self.0.name()
    }

    fn version(&self) -> &str {
        self.0.version()
    }

    fn capabilities(&self) -> &[CapabilityId] {
        self.0.capabilities()
    }

    fn requires(&self) -> &[CapabilityId] {
        self.0.requires()
    }

    fn optional(&self) -> &[CapabilityId] {
        self.0.optional()
    }

    fn metadata(&self, cap: &CapabilityId) -> Option<CapabilityMetadata> {
        self.0.metadata(cap)
    }

    fn contribute(&self, ctx: &mut handl_platform::ContributionContext) -> Result<(), PlatformError> {
        self.0.contribute(ctx)
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        self.0.run(request, ctx)
    }
}
