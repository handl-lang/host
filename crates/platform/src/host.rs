use std::collections::HashMap;

use crate::error::PlatformError;
use crate::extension::{Extension, ExtensionRegistry};
use crate::kit::{Kit, KitManifest, KitRegistry};
use crate::language::{LanguagePipeline, PipelineOutput};
use crate::manifest::PluginManifest;
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

/// A host for running language pipelines composed from a kit.
///
/// Concrete language plugins are registered by plugin id as factory functions
/// that call `register_all` on a `LanguagePipeline`. This keeps `handl-platform`
/// decoupled from the concrete plugin crates.
#[derive(Default)]
pub struct LanguageHost {
    factories: HashMap<String, fn(&mut LanguagePipeline)>,
}

impl LanguageHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a factory for a language plugin id.
    pub fn register_plugin(&mut self, id: impl Into<String>, factory: fn(&mut LanguagePipeline)) {
        self.factories.insert(id.into(), factory);
    }

    /// Resolve the kit, initialize the pipeline in dependency order, and run
    /// lex / parse / format / diagnose.
    pub fn run_language_pipeline(
        &self,
        kit: &Kit,
        source: &str,
    ) -> Result<PipelineOutput, PlatformError> {
        let resolved = kit.resolve().map_err(|e| PlatformError::Plugin(e.to_string()))?;
        let mut pipeline = LanguagePipeline::new();

        // Register in reverse initialization order so that more specific
        // plugins (e.g. doc-comments) get first pick during parsing.
        for id in resolved.initialization_order.iter().rev() {
            let factory = self.factories.get(id).copied().ok_or_else(|| {
                PlatformError::Plugin(format!(
                    "no language plugin factory registered for {id}"
                ))
            })?;
            factory(&mut pipeline);
        }

        let (ast, formatted, diagnostics) = pipeline.run(source);
        Ok(PipelineOutput {
            formatted,
            diagnostics,
            ast,
        })
    }

    /// Convenience helper to load a kit manifest and wrap it in a `Kit`.
    pub fn load_kit(
        &self,
        path: impl AsRef<std::path::Path>,
        available_plugins: Vec<PluginManifest>,
    ) -> Result<Kit, PlatformError> {
        let manifest = KitManifest::load(path)?;
        Ok(Kit {
            manifest,
            available_plugins,
        })
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
