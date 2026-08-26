//! Host implementations that coordinate plugins, kits, and extensions.

use std::collections::HashMap;

#[cfg(feature = "concurrent")]
use handl_vm_common::ModuleHost;

#[cfg(feature = "concurrent")]
use handl_lib_concurrent::ConcurrencyModule;

use crate::error::PlatformError;
use crate::extension::{Extension, ExtensionRegistry};
use crate::kit::{Kit, KitManifest, KitRegistry};
use crate::language::{LanguagePipeline, PipelineOutput};
use crate::manifest::PluginManifest;
use crate::plugin::ProcessPlugin;
use crate::plugin::{CapabilityRequest, CapabilityResponse, Plugin, PluginContext, PluginRegistry};

/// A host that exposes the plugin, kit, and extension registries.
pub trait Host: Send + Sync {
    /// Returns the plugin registry.
    fn plugins(&self) -> &PluginRegistry;
    /// Returns the kit registry.
    fn kits(&self) -> &KitRegistry;
    /// Returns the extension registry.
    fn extensions(&self) -> &ExtensionRegistry;
}

/// A concrete host with mutable access to plugins, kits, and extensions.
pub struct DefaultHost {
    plugins: PluginRegistry,
    kits: KitRegistry,
    extensions: ExtensionRegistry,
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
            extensions: ExtensionRegistry::default(),
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

    /// Returns a mutable reference to the extension registry.
    pub fn extensions_mut(&mut self) -> &mut ExtensionRegistry {
        &mut self.extensions
    }

    /// Registers a plugin.
    pub fn register_plugin<P: Plugin + 'static>(&mut self, plugin: P) {
        self.plugins.register(plugin);
    }

    /// Registers a kit.
    pub fn register_kit(&mut self, kit: Kit) {
        self.kits.register(kit);
    }

    /// Registers an extension.
    pub fn register_extension<E: Extension + 'static>(&mut self, extension: E) {
        self.extensions.register(extension);
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

    /// Runs the request against the plugin that provides the requested capability.
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

impl Default for DefaultHost {
    fn default() -> Self {
        Self::new()
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
    /// Creates a new, empty language host.
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
        let resolved = kit
            .resolve()
            .map_err(|e| PlatformError::Plugin(e.to_string()))?;
        let mut pipeline = LanguagePipeline::new();

        // Register in reverse initialization order so that more specific
        // plugins (e.g. doc-comments) get first pick during parsing.
        for id in resolved.initialization_order.iter().rev() {
            let factory = self.factories.get(id).copied().ok_or_else(|| {
                PlatformError::Plugin(format!("no language plugin factory registered for {id}"))
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
    use crate::{
        CapabilityRequest, CapabilityResponse, InProcessPlugin, Kit, KitManifest, PlatformError,
        PluginContext, PluginManifest,
    };

    #[derive(Debug)]
    struct MockExt {
        id: &'static str,
        meta: serde_json::Value,
    }

    impl Extension for MockExt {
        fn id(&self) -> &str {
            self.id
        }

        fn metadata(&self) -> serde_json::Value {
            self.meta.clone()
        }
    }

    #[test]
    fn default_host_initializes() {
        let host = DefaultHost::new();
        assert!(host.plugins().capabilities().is_empty());
        assert!(host.kits().all().is_empty());
        assert!(host.extensions().all().is_empty());
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
        host.extensions_mut().register(MockExt {
            id: "x",
            meta: serde_json::Value::Null,
        });

        assert!(!host.plugins().capabilities().is_empty());
        assert_eq!(host.kits().all().len(), 1);
        assert_eq!(host.extensions().all().len(), 1);

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

    #[test]
    fn language_host_loads_and_runs_kit() {
        let dir = std::env::temp_dir();
        let path = dir.join("handl_language_host_kit.toml");
        std::fs::write(
            &path,
            r#"id = "core"
name = "core"
plugins = ["word"]
"#,
        )
        .unwrap();

        let mut host = LanguageHost::new();
        host.register_plugin("word", |_pipeline| {});

        let available = vec![PluginManifest {
            id: "word".into(),
            version: "0.1.0".into(),
            provides: vec![],
            requires: vec![],
            optional: vec![],
            extends: vec![],
            conflicts: vec![],
            replaces: vec![],
            targets: vec![],
            exclusive: vec![],
            metadata: serde_json::Value::Null,
        }];

        let kit = host.load_kit(&path, available).unwrap();
        let output = host.run_language_pipeline(&kit, "hello").unwrap();
        assert_eq!(output.formatted, "");
        assert!(output.diagnostics.is_empty());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn language_host_errors_for_missing_factory() {
        let host = LanguageHost::new();
        let mut kit = Kit {
            manifest: KitManifest {
                id: "core".into(),
                name: "core".into(),
                plugins: vec!["unregistered".into()],
                target: None,
                selectors: vec![],
            },
            available_plugins: vec![PluginManifest {
                id: "unregistered".into(),
                version: "0.1.0".into(),
                provides: vec![],
                requires: vec![],
                optional: vec![],
                extends: vec![],
                conflicts: vec![],
                replaces: vec![],
                targets: vec![],
                exclusive: vec![],
                metadata: serde_json::Value::Null,
            }],
        };
        let err = host.run_language_pipeline(&kit, "x").unwrap_err();
        assert!(matches!(err, PlatformError::Plugin(_)));

        kit.manifest.plugins = vec![];
        let output = host.run_language_pipeline(&kit, "x").unwrap();
        assert!(output.ast.is_empty());
    }

    #[test]
    fn language_host_load_kit_fails_for_missing_file() {
        let host = LanguageHost::new();
        let path = std::env::temp_dir().join("handl_kit_does_not_exist.toml");
        let err = host.load_kit(&path, vec![]).unwrap_err();
        assert!(matches!(err, PlatformError::InvalidMetadata(_)));
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
