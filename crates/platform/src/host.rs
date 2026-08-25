use crate::error::PlatformError;
use crate::extension::{Extension, ExtensionRegistry};
use crate::kit::{Kit, KitRegistry};
use crate::plugin::{CapabilityRequest, CapabilityResponse, Plugin, PluginContext, PluginRegistry};
use crate::plugin::ProcessPlugin;

pub trait Host: Send + Sync {
    fn plugins(&self) -> &PluginRegistry;
    fn kits(&self) -> &KitRegistry;
    fn extensions(&self) -> &ExtensionRegistry;
}

#[derive(Default)]
pub struct DefaultHost {
    plugins: PluginRegistry,
    kits: KitRegistry,
    extensions: ExtensionRegistry,
}

impl DefaultHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plugins_mut(&mut self) -> &mut PluginRegistry {
        &mut self.plugins
    }

    pub fn kits_mut(&mut self) -> &mut KitRegistry {
        &mut self.kits
    }

    pub fn extensions_mut(&mut self) -> &mut ExtensionRegistry {
        &mut self.extensions
    }

    pub fn register_plugin<P: Plugin + 'static>(&mut self, plugin: P) {
        self.plugins.register(plugin);
    }

    pub fn register_kit(&mut self, kit: Kit) {
        self.kits.register(kit);
    }

    pub fn register_extension<E: Extension + 'static>(&mut self, extension: E) {
        self.extensions.register(extension);
    }

    pub fn register_process_plugin(
        &mut self,
        name: impl Into<String>,
        capabilities: &[crate::capability::CapabilityId],
    ) {
        self.plugins.register(ProcessPlugin::new(name, capabilities));
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

impl Host for DefaultHost {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_host_initializes() {
        let host = DefaultHost::new();
        assert!(host.plugins().capabilities().is_empty());
        assert!(host.kits().all().is_empty());
        assert!(host.extensions().all().is_empty());
    }
}
