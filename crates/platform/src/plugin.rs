use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::capability::{CapabilityId, CapabilityMetadata};
use crate::error::PlatformError;
use crate::extension::ExtensionRegistry;
use crate::kit::KitRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub capability: CapabilityId,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct CapabilityResponse(pub serde_json::Value);

pub struct PluginContext<'a> {
    pub plugins: &'a mut PluginRegistry,
    pub kits: &'a mut KitRegistry,
    pub extensions: &'a mut ExtensionRegistry,
}

pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> &[CapabilityId];
    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata>;
    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError>;
}

#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn Plugin + 'static>>,
    by_capability: HashMap<CapabilityId, usize>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<P: Plugin + 'static>(&mut self, plugin: P) {
        let idx = self.plugins.len();
        for cap in plugin.capabilities() {
            self.by_capability.insert(cap.clone(), idx);
        }
        self.plugins.push(Arc::new(plugin));
    }

    pub fn resolve(&self, capability: &CapabilityId) -> Option<Arc<dyn Plugin>> {
        let idx = *self.by_capability.get(capability)?;
        Some(self.plugins[idx].clone())
    }

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

    pub fn remove(&mut self, _name: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotImplemented)
    }

    pub fn names(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.name().to_string()).collect()
    }
}

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

pub struct ProcessPlugin {
    name: String,
    capabilities: Vec<CapabilityId>,
}

impl ProcessPlugin {
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

    fn echo() -> impl Fn(&CapabilityRequest, &mut PluginContext) -> Result<CapabilityResponse, PlatformError>
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
        let (mut plugins, mut kits, mut extensions) = (PluginRegistry::new(), KitRegistry::new(), ExtensionRegistry::new());
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
        let (mut plugins, mut kits, mut extensions) = (PluginRegistry::new(), KitRegistry::new(), ExtensionRegistry::new());
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
}
