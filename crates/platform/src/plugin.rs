//! Plugin traits, registries, and built-in plugin implementations.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::capability::{CapabilityId, CapabilityMetadata};
use crate::error::PlatformError;
use crate::extension::ExtensionRegistry;
use crate::kit::KitRegistry;

/// A request to invoke a capability on a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    /// The requested capability.
    pub capability: CapabilityId,
    /// The request payload.
    pub payload: serde_json::Value,
}

/// A response payload returned by a plugin.
#[derive(Debug, Clone)]
pub struct CapabilityResponse(pub serde_json::Value);

/// Context passed to a plugin during execution.
pub struct PluginContext<'a> {
    /// The plugin registry.
    pub plugins: &'a mut PluginRegistry,
    /// The kit registry.
    pub kits: &'a mut KitRegistry,
    /// The extension registry.
    pub extensions: &'a mut ExtensionRegistry,
}

/// A plugin that can provide and run capabilities.
pub trait Plugin: Send + Sync {
    /// Returns the plugin's display name.
    fn name(&self) -> &str;
    /// Returns the capabilities provided by this plugin.
    fn capabilities(&self) -> &[CapabilityId];
    /// Returns metadata for a capability, if available.
    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata>;
    /// Runs the requested capability.
    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError>;
}

/// A registry of loaded plugins indexed by capability.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn Plugin + 'static>>,
    by_capability: HashMap<CapabilityId, usize>,
}

impl PluginRegistry {
    /// Creates an empty plugin registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a plugin and indexes its capabilities.
    pub fn register<P: Plugin + 'static>(&mut self, plugin: P) {
        let idx = self.plugins.len();
        for cap in plugin.capabilities() {
            self.by_capability.insert(cap.clone(), idx);
        }
        self.plugins.push(Arc::new(plugin));
    }

    /// Returns the plugin that provides the given capability, if any.
    pub fn resolve(&self, capability: &CapabilityId) -> Option<Arc<dyn Plugin>> {
        let idx = *self.by_capability.get(capability)?;
        Some(self.plugins[idx].clone())
    }

    /// Returns all capabilities with their metadata.
    pub fn capabilities(&self) -> Vec<(CapabilityId, CapabilityMetadata)> {
        let mut out = Vec::new();
        for plugin in &self.plugins {
            for cap in plugin.capabilities() {
                if let Some(meta) = plugin.metadata(cap) {
                    out.push((cap.clone(), meta));
                }
            }
        }
        out
    }

    /// Runs the requested capability through the providing plugin.
    pub fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        match self.resolve(&request.capability) {
            Some(plugin) => plugin.run(request, ctx),
            None => Err(PlatformError::UnknownCapability(request.capability.clone())),
        }
    }

    /// Removes a plugin by name.
    pub fn remove(&mut self, name: &str) -> Result<(), PlatformError> {
        let len_before = self.plugins.len();
        self.plugins.retain(|p| p.name() != name);
        if self.plugins.len() == len_before {
            return Err(PlatformError::InvalidMetadata(format!(
                "plugin not found: {name}"
            )));
        }
        self.by_capability.clear();
        for (idx, p) in self.plugins.iter().enumerate() {
            for cap in p.capabilities() {
                self.by_capability.insert(cap.clone(), idx);
            }
        }
        Ok(())
    }

    /// Returns the names of all registered plugins.
    pub fn names(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.name().to_string()).collect()
    }
}

/// A plugin that runs in the host process using a closure.
pub struct InProcessPlugin<F>
where
    F: Fn(&CapabilityRequest, &mut PluginContext) -> Result<CapabilityResponse, PlatformError>
        + Send
        + Sync
        + 'static,
{
    name: String,
    capabilities: Vec<CapabilityId>,
    metadata: CapabilityMetadata,
    run_fn: F,
}

impl<F> InProcessPlugin<F>
where
    F: Fn(&CapabilityRequest, &mut PluginContext) -> Result<CapabilityResponse, PlatformError>
        + Send
        + Sync
        + 'static,
{
    /// Creates a new in-process plugin.
    pub fn new(
        name: impl Into<String>,
        capabilities: &[CapabilityId],
        metadata: CapabilityMetadata,
        run_fn: F,
    ) -> Self {
        Self {
            name: name.into(),
            capabilities: capabilities.to_vec(),
            metadata,
            run_fn,
        }
    }
}

impl<F> Plugin for InProcessPlugin<F>
where
    F: Fn(&CapabilityRequest, &mut PluginContext) -> Result<CapabilityResponse, PlatformError>
        + Send
        + Sync
        + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| self.metadata.clone())
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        (self.run_fn)(request, ctx)
    }
}

/// A placeholder plugin that represents an out-of-process capability provider.
pub struct ProcessPlugin {
    name: String,
    capabilities: Vec<CapabilityId>,
}

impl ProcessPlugin {
    /// Creates a new process plugin placeholder.
    pub fn new(name: impl Into<String>, capabilities: &[CapabilityId]) -> Self {
        Self {
            name: name.into(),
            capabilities: capabilities.to_vec(),
        }
    }
}

impl Plugin for ProcessPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo()
    -> impl Fn(&CapabilityRequest, &mut PluginContext) -> Result<CapabilityResponse, PlatformError>
    {
        |req: &CapabilityRequest, _ctx: &mut PluginContext| {
            Ok(CapabilityResponse(serde_json::json!({
                "echoed": req.capability.as_str(),
                "payload": req.payload,
            })))
        }
    }

    fn test_ctx<'a>(
        plugins: &'a mut PluginRegistry,
        kits: &'a mut KitRegistry,
        extensions: &'a mut ExtensionRegistry,
    ) -> PluginContext<'a> {
        PluginContext {
            plugins,
            kits,
            extensions,
        }
    }

    #[test]
    fn in_process_plugin_runs_and_reports_metadata() {
        let p = InProcessPlugin::new(
            "echo",
            &["echo".into()],
            serde_json::json!({"command":"echo"}),
            echo(),
        );
        assert_eq!(p.name(), "echo");
        assert_eq!(p.capabilities(), &[CapabilityId::from("echo")]);
        assert!(p.metadata(&"echo".into()).is_some());
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = test_ctx(&mut plugins, &mut kits, &mut extensions);
        let resp = p.run(
            &CapabilityRequest {
                capability: "echo".into(),
                payload: serde_json::json!({"x":1}),
            },
            &mut ctx,
        );
        assert!(resp.is_ok());
    }

    #[test]
    fn process_plugin_returns_not_implemented() {
        let p = ProcessPlugin::new("proc", &["remote".into()]);
        assert_eq!(p.name(), "proc");
        assert!(p.metadata(&"remote".into()).is_none());
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = test_ctx(&mut plugins, &mut kits, &mut extensions);
        let res = p.run(
            &CapabilityRequest {
                capability: "remote".into(),
                payload: serde_json::Value::Null,
            },
            &mut ctx,
        );
        assert!(matches!(res, Err(PlatformError::NotImplemented)));
    }

    #[test]
    fn registry_lists_capabilities_and_dispatches() {
        let mut reg = PluginRegistry::new();
        reg.register(InProcessPlugin::new(
            "echo",
            &["echo".into()],
            serde_json::json!({"command":"echo"}),
            echo(),
        ));
        reg.register(InProcessPlugin::new(
            "upper",
            &["shout".into()],
            serde_json::json!({"command":"shout"}),
            |_req, _ctx| Ok(CapabilityResponse(serde_json::json!("HELLO"))),
        ));

        let caps = reg.capabilities();
        assert_eq!(caps.len(), 2);

        let resolved = reg.resolve(&"echo".into());
        assert!(resolved.is_some());

        let (mut kits, mut extensions) = (KitRegistry::new(), ExtensionRegistry::new());
        let mut ctx = test_ctx(&mut reg, &mut kits, &mut extensions);
        let plugin = ctx.plugins.resolve(&"shout".into()).unwrap();
        let response = plugin.run(
            &CapabilityRequest {
                capability: "shout".into(),
                payload: serde_json::Value::Null,
            },
            &mut ctx,
        );
        assert!(response.is_ok());
    }

    #[test]
    fn registry_unknown_capability() {
        let mut reg = PluginRegistry::new();
        let (mut kits, mut extensions) = (KitRegistry::new(), ExtensionRegistry::new());
        let ctx = test_ctx(&mut reg, &mut kits, &mut extensions);
        assert!(ctx.plugins.resolve(&"missing".into()).is_none());
    }

    #[test]
    fn in_process_plugin_metadata_for_missing_capability() {
        let p = InProcessPlugin::new("echo", &["echo".into()], serde_json::json!({}), echo());
        assert!(p.metadata(&"missing".into()).is_none());
    }

    #[test]
    fn process_plugin_and_registry_misc() {
        let mut reg = PluginRegistry::new();
        reg.register(ProcessPlugin::new("proc", &["a".into(), "b".into()]));

        assert!(reg.names().contains(&"proc".to_string()));
        assert!(reg.remove("proc").is_ok());
        assert!(!reg.names().contains(&"proc".to_string()));
        assert!(reg.resolve(&"a".into()).is_none());
        assert!(matches!(
            reg.remove("missing"),
            Err(PlatformError::InvalidMetadata(_))
        ));
    }

    #[test]
    fn registry_run_dispatches_and_reports_unknown() {
        let mut reg = PluginRegistry::new();
        reg.register(InProcessPlugin::new(
            "echo",
            &["echo".into()],
            serde_json::json!({}),
            echo(),
        ));
        let (mut kits, mut extensions) = (KitRegistry::new(), ExtensionRegistry::new());
        let mut dummy_plugins = PluginRegistry::new();
        let mut ctx = test_ctx(&mut dummy_plugins, &mut kits, &mut extensions);

        let req = CapabilityRequest {
            capability: "echo".into(),
            payload: serde_json::Value::Null,
        };
        let resp = reg.run(&req, &mut ctx).unwrap();
        assert!(resp.0.get("echoed").is_some());

        let missing = CapabilityRequest {
            capability: "missing".into(),
            payload: serde_json::Value::Null,
        };
        let res = reg.run(&missing, &mut ctx);
        assert!(matches!(res, Err(PlatformError::UnknownCapability(_))));
    }
}
