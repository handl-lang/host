use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, Extension,
    ExtensionRegistry, Host, Kit, KitRegistry, PlatformError, Plugin, PluginContext, PluginRegistry,
};

pub struct MockPlugin {
    name: String,
    capabilities: Vec<CapabilityId>,
    output: serde_json::Value,
}

impl MockPlugin {
    pub fn new(name: impl Into<String>, capabilities: &[&str]) -> Self {
        Self {
            name: name.into(),
            capabilities: capabilities.iter().map(|&c| c.into()).collect(),
            output: serde_json::Value::Null,
        }
    }

    pub fn with_output(mut self, output: serde_json::Value) -> Self {
        self.output = output;
        self
    }
}

impl Plugin for MockPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({"plugin": self.name, "capability": capability.as_str()}))
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Ok(CapabilityResponse(self.output.clone()))
    }
}

pub struct MockHost {
    pub plugins: PluginRegistry,
    pub kits: KitRegistry,
    pub extensions: ExtensionRegistry,
}

impl MockHost {
    pub fn new() -> Self {
        Self {
            plugins: PluginRegistry::new(),
            kits: KitRegistry::new(),
            extensions: ExtensionRegistry::new(),
        }
    }

    pub fn load_kit(&mut self, kit: Kit) {
        for plugin_name in &kit.plugins {
            self.plugins
                .register(MockPlugin::new(plugin_name, &[plugin_name]).with_output(
                    serde_json::json!({"kit": kit.name, "plugin": plugin_name}),
                ));
        }
        self.kits.register(kit);
    }

    pub fn run(
        &mut self,
        request: &CapabilityRequest,
    ) -> Result<CapabilityResponse, PlatformError> {
        let mut ctx = PluginContext {
            plugins: &mut self.plugins,
            kits: &mut self.kits,
            extensions: &mut self.extensions,
        };
        let plugin = ctx
            .plugins
            .resolve(&request.capability)
            .ok_or_else(|| PlatformError::UnknownCapability(request.capability.clone()))?;
        plugin.run(request, &mut ctx)
    }
}

impl Host for MockHost {
    fn plugins(&self) -> &PluginRegistry {
        &self.plugins
    }

    fn kits(&self) -> &KitRegistry {
        &self.kits
    }

    fn extensions(&self) -> &ExtensionRegistry {
        &self.extensions
    }
}

#[derive(Debug)]
pub struct MockExtension {
    id: &'static str,
    metadata: serde_json::Value,
}

impl MockExtension {
    pub fn new(id: &'static str, metadata: serde_json::Value) -> Self {
        Self { id, metadata }
    }
}

impl Extension for MockExtension {
    fn id(&self) -> &str {
        self.id
    }

    fn metadata(&self) -> serde_json::Value {
        self.metadata.clone()
    }
}
