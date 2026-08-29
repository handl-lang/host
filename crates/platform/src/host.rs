//! Host implementations that coordinate plugins and kits.


#[cfg(feature = "concurrent")]
use handl_plugin_vm_common::ModuleHost;

#[cfg(feature = "concurrent")]
use handl_plugin_lib_concurrent::ConcurrencyModule;

use crate::error::PlatformError;
use crate::kit::{Kit, KitRegistry};
use crate::plugin::ProcessPlugin;
use crate::plugin::{CapabilityRequest, CapabilityResponse, Plugin, PluginContext, PluginRegistry};
use crate::plugin_model::ContributionContext;

/// A host that exposes the plugin and kit registries.
pub trait Host: Send + Sync {
    /// Returns the plugin registry.
    fn plugins(&self) -> &PluginRegistry;
    /// Returns the kit registry.
    fn kits(&self) -> &KitRegistry;
}

/// A concrete host with mutable access to plugins and kits.
pub struct DefaultHost {
    plugins: PluginRegistry,
    kits: KitRegistry,
    #[cfg(feature = "concurrent")]
    modules: ModuleHost,
}

impl DefaultHost {
    /// Creates a new, empty default host.
    pub fn new() -> Self {
        #[cfg(feature = "concurrent")]
        let mut modules = ModuleHost::new();

        #[cfg(feature = "concurrent")]
        {
            modules = modules.with(ConcurrencyModule::new());
        }

        Self {
            plugins: PluginRegistry::default(),
            kits: KitRegistry::default(),
            #[cfg(feature = "concurrent")]
            modules,
        }
    }

    /// Returns `true` if `name` resolves to a function in the module host.
    #[allow(unused_variables)]
    pub fn resolve(&self, name: &str) -> bool {
        #[cfg(feature = "concurrent")]
        {
            return self.modules.function(name).is_some();
        }

        #[cfg(not(feature = "concurrent"))]
        {
            false
        }
    }

    /// Returns a mutable reference to the plugin registry.
    pub fn plugins_mut(&mut self) -> &mut PluginRegistry {
        &mut self.plugins
    }

    /// Returns a mutable reference to the kit registry.
    pub fn kits_mut(&mut self) -> &mut KitRegistry {
        &mut self.kits
    }

    /// Registers a plugin.
    pub fn register_plugin<P: Plugin + 'static>(&mut self, plugin: P) {
        self.plugins.register(plugin);
    }

    /// Registers a kit.
    pub fn register_kit(&mut self, kit: Kit) {
        self.kits.register(kit);
    }

    /// Registers a process-backed plugin by name and capabilities.
    pub fn register_process_plugin(
        &mut self,
        name: impl Into<String>,
        capabilities: &[crate::capability::CapabilityId],
    ) {
        self.plugins
            .register(ProcessPlugin::new(name, capabilities));
    }

    /// Loads all registered plugins to a fixed point, repeatedly calling
    /// `contribute` until a full pass registers no new plugins.
    pub fn load(&mut self) -> Result<(), PlatformError> {
        loop {
            let count = self.plugins.len();
            let mut changed = false;
            for i in 0..count {
                let plugin = match self.plugins.take_at(i) {
                    Some(p) => p,
                    None => continue,
                };
                let result = {
                    let mut ctx = ContributionContext::new(&mut self.plugins);
                    plugin.contribute(&mut ctx)
                };
                self.plugins.replace_at(i, plugin);
                result?;
                if self.plugins.len() > count {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(())
    }

    /// Runs the request against the plugin that provides the requested capability.
    pub fn run(
        &mut self,
        request: &CapabilityRequest,
    ) -> Result<CapabilityResponse, PlatformError> {
        let mut ctx = PluginContext {
            plugins: &mut self.plugins,
            kits: &mut self.kits,
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
}

impl Default for DefaultHost {
    fn default() -> Self {
        Self::new()
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityRequest, CapabilityResponse, InProcessPlugin, Kit, KitManifest, PlatformError,
        PluginContext,
    };

    #[test]
    fn default_host_initializes() {
        let host = DefaultHost::new();
        assert!(host.plugins().capabilities().is_empty());
        assert!(host.kits().all().is_empty());
    }

    #[test]
    fn default_host_lifecycle() {
        let mut host = DefaultHost::new();
        host.register_process_plugin("remote", &["remote".into()]);
        let echo = |req: &CapabilityRequest,
                    _ctx: &mut PluginContext|
         -> Result<CapabilityResponse, PlatformError> {
            Ok(CapabilityResponse(serde_json::json!({
                "echoed": req.capability.as_str(),
            })))
        };
        host.register_plugin(InProcessPlugin::new(
            "echo",
            &["echo".into()],
            serde_json::Value::Null,
            echo,
        ));
        host.kits_mut().register(Kit {
            manifest: KitManifest {
                id: "k".into(),
                name: "k".into(),
                plugins: vec![],
                target: None,
                selectors: vec![],
            },
            available_plugins: vec![],
        });

        assert!(!host.plugins().capabilities().is_empty());
        assert_eq!(host.kits().all().len(), 1);

        let resp = host
            .run(&CapabilityRequest {
                capability: "echo".into(),
                payload: serde_json::Value::Null,
            })
            .unwrap();
        assert!(resp.0.get("echoed").is_some());

        assert!(matches!(
            host.run(&CapabilityRequest {
                capability: "missing".into(),
                payload: serde_json::Value::Null,
            }),
            Err(PlatformError::UnknownCapability(_))
        ));
    }


    #[cfg(feature = "concurrent")]
    #[test]
    fn concurrent_spawn_resolves_with_feature() {
        let host = DefaultHost::new();
        assert!(host.resolve("concurrent.spawn"));
    }

    #[cfg(not(feature = "concurrent"))]
    #[test]
    fn concurrent_spawn_unavailable_without_feature() {
        let host = DefaultHost::new();
        assert!(!host.resolve("concurrent.spawn"));
    }
}
