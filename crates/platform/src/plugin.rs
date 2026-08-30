//! Plugin traits, registries, and built-in plugin implementations.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::capability::{CapabilityId, CapabilityMetadata};
use crate::error::PlatformError;
use crate::kit::KitRegistry;
use crate::plugin_model::ContributionContext;

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
}

/// A plugin that can provide and run capabilities.
pub trait Plugin: Send + Sync + Any {
    /// Returns the plugin's stable id.
    fn id(&self) -> &str {
        self.name()
    }

    /// Returns the plugin's display name.
    fn name(&self) -> &str;

    /// Returns the plugin's version.
    fn version(&self) -> &str {
        "0.0.0"
    }

    /// Returns the capabilities provided by this plugin.
    fn capabilities(&self) -> &[CapabilityId];

    /// Returns the plugin's required dependency capabilities.
    fn requires(&self) -> &[CapabilityId] {
        &[]
    }

    /// Returns the plugin's optional dependency capabilities.
    fn optional(&self) -> &[CapabilityId] {
        &[]
    }

    /// Returns metadata for a capability, if available.
    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata>;

    /// Returns the plugin's manifest, if it has one.
    ///
    /// Plugins that were loaded from a manifest or want to participate in
    /// composition resolution should return it here. Plugins that are
    /// built-in and have no manifest data return `None` (the default).
    fn manifest(&self) -> Option<&crate::manifest::PluginManifest> {
        None
    }

    /// Contributes rules or other plugins during host loading.
    fn contribute(&self, _ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        Ok(())
    }

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
    plugins: Vec<Option<Arc<dyn Plugin + 'static>>>,
    by_capability: HashMap<CapabilityId, usize>,
    pending_excludes: Vec<String>,
    pending_rules: Vec<PendingRule>,
}

/// Outcome of retrying a deferred rule contribution.
pub(crate) enum PendingStatus {
    /// The rule was accepted by its `RuleAcceptor`.
    Applied,
    /// The acceptor is still not registered; retry later.
    StillMissing,
    /// The acceptor was found but rejected the rule.
    Failed(PlatformError),
}

/// A rule contribution deferred because its `RuleAcceptor` was not yet
/// registered when `contribute` was called. Retried as loading progresses.
pub(crate) struct PendingRule {
    pub(crate) type_name: String,
    pub(crate) try_apply: Box<dyn FnMut(&mut PluginRegistry) -> PendingStatus + Send + Sync>,
}

impl PluginRegistry {
    /// Creates an empty plugin registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Returns `true` if no plugins are registered.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Temporarily take a plugin by index, leaving an empty slot.
    pub fn take_at(&mut self, index: usize) -> Option<Arc<dyn Plugin + 'static>> {
        self.plugins.get_mut(index)?.take()
    }

    /// Place a plugin back into a previously emptied slot.
    pub fn replace_at(&mut self, index: usize, plugin: Arc<dyn Plugin + 'static>) {
        if let Some(slot) = self.plugins.get_mut(index) {
            *slot = Some(plugin);
        }
    }

    /// Registers a plugin and indexes its capabilities.
    pub fn register<P: Plugin + 'static>(&mut self, plugin: P) {
        let idx = self.plugins.len();
        for cap in plugin.capabilities() {
            self.by_capability.insert(cap.clone(), idx);
        }
        self.plugins.push(Some(Arc::new(plugin)));
    }

    /// Returns the plugin that provides the given capability, if any.
    pub fn resolve(&self, capability: &CapabilityId) -> Option<Arc<dyn Plugin>> {
        let idx = *self.by_capability.get(capability)?;
        self.plugins.get(idx).cloned().flatten()
    }

    /// Returns all capabilities with their metadata.
    pub fn capabilities(&self) -> Vec<(CapabilityId, CapabilityMetadata)> {
        let mut out = Vec::new();
        for plugin in self.plugins.iter().flatten() {
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
        self.plugins
            .retain(|p| p.as_ref().map_or(true, |p| p.name() != name));
        if self.plugins.len() == len_before {
            return Err(PlatformError::InvalidMetadata(format!(
                "plugin not found: {name}"
            )));
        }
        self.reindex();
        Ok(())
    }

    /// Removes the plugin with the given id, returning `true` if removed.
    pub fn remove_by_id(&mut self, id: &str) -> bool {
        let len_before = self.plugins.len();
        self.plugins
            .retain(|p| p.as_ref().map_or(true, |p| p.id() != id));
        let removed = self.plugins.len() < len_before;
        if removed {
            self.reindex();
        }
        removed
    }

    /// Schedules a plugin id for exclusion after all plugins have loaded.
    pub fn pending_exclude(&mut self, id: String) {
        self.pending_excludes.push(id);
    }

    /// Removes all plugins scheduled via `pending_exclude`.
    pub fn apply_pending_excludes(&mut self) {
        if self.pending_excludes.is_empty() {
            return;
        }
        let excludes = std::mem::take(&mut self.pending_excludes);
        for id in &excludes {
            self.remove_by_id(id);
        }
    }

    /// Queues a deferred rule contribution for later retry.
    pub(crate) fn push_pending_rule(&mut self, rule: PendingRule) {
        self.pending_rules.push(rule);
    }

    /// Retries all deferred rule contributions. Rules whose acceptor has
    /// appeared are applied; the rest remain queued. Returns the first
    /// acceptor-rejection error encountered.
    pub(crate) fn flush_pending_rules(&mut self) -> Result<(), PlatformError> {
        if self.pending_rules.is_empty() {
            return Ok(());
        }
        let pending = std::mem::take(&mut self.pending_rules);
        let mut still_pending = Vec::new();
        for mut rule in pending {
            match (rule.try_apply)(self) {
                PendingStatus::Applied => {}
                PendingStatus::StillMissing => still_pending.push(rule),
                PendingStatus::Failed(e) => {
                    self.pending_rules = still_pending;
                    return Err(e);
                }
            }
        }
        self.pending_rules = still_pending;
        Ok(())
    }

    /// Returns the type names of acceptors that still have unresolved
    /// deferred rules.
    pub(crate) fn pending_rule_acceptors(&self) -> Vec<String> {
        self.pending_rules
            .iter()
            .map(|r| r.type_name.clone())
            .collect()
    }

    /// Returns manifests for all registered plugins.
    ///
    /// Plugins that return a manifest from `Plugin::manifest` use it directly.
    /// Others get a synthesized manifest from their id, version, and capabilities.
    pub fn iter_manifests(&self) -> Vec<crate::manifest::PluginManifest> {
        self.plugins
            .iter()
            .flatten()
            .map(|p| {
                if let Some(m) = p.manifest() {
                    m.clone()
                } else {
                    crate::manifest::PluginManifest {
                        id: p.id().to_string(),
                        version: p.version().to_string(),
                        provides: p.capabilities().to_vec(),
                        requires: Vec::new(),
                        optional: Vec::new(),
                        extends: Vec::new(),
                        conflicts: Vec::new(),
                        replaces: Vec::new(),
                        targets: Vec::new(),
                        exclusive: Vec::new(),
                        metadata: serde_json::Value::Null,
                        ast_kinds: Vec::new(),
                    }
                }
            })
            .collect()
    }

    /// Rebuilds the capability index from the current plugin list.
    fn reindex(&mut self) {
        self.by_capability.clear();
        for (idx, p) in self.plugins.iter().enumerate() {
            if let Some(p) = p {
                for cap in p.capabilities() {
                    self.by_capability.insert(cap.clone(), idx);
                }
            }
        }
    }

    /// Returns the names of all registered plugins.
    pub fn names(&self) -> Vec<String> {
        self.plugins
            .iter()
            .flatten()
            .map(|p| p.name().to_string())
            .collect()
    }

    /// Returns the ids of all registered plugins.
    pub fn ids(&self) -> Vec<String> {
        self.plugins
            .iter()
            .flatten()
            .map(|p| p.id().to_string())
            .collect()
    }

    /// Returns `true` if a plugin with the given id is registered.
    pub fn has_id(&self, id: &str) -> bool {
        self.plugins.iter().flatten().any(|p| p.id() == id)
    }

    /// Returns a mutable reference to a registered plugin of concrete type `P`, if any.
    pub fn get_plugin_mut<P: Plugin + 'static>(&mut self) -> Option<&mut P> {
        for arc in self.plugins.iter_mut().flatten() {
            if let Some(plugin) = Arc::get_mut(arc) {
                let any = plugin as &mut dyn Any;
                if let Some(concrete) = any.downcast_mut::<P>() {
                    return Some(concrete);
                }
            }
        }
        None
    }

    /// Returns a shared reference to a registered plugin of concrete type `P`, if any.
    pub fn get_plugin<P: Plugin + 'static>(&self) -> Option<&P> {
        for arc in self.plugins.iter().flatten() {
            let any = arc.as_ref() as &dyn Any;
            if let Some(concrete) = any.downcast_ref::<P>() {
                return Some(concrete);
            }
        }
        None
    }

    /// Returns a mutable reference to a registered plugin with the given name, if any.
    pub fn get_plugin_by_name_mut(&mut self, name: &str) -> Option<&mut (dyn Plugin + 'static)> {
        for arc in self.plugins.iter_mut().flatten() {
            if arc.name() == name {
                return Arc::get_mut(arc).map(|p| p as &mut (dyn Plugin + 'static));
            }
        }
        None
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

type RunFn = Box<
    dyn Fn(&CapabilityRequest, &mut PluginContext) -> Result<CapabilityResponse, PlatformError>
        + Send
        + Sync,
>;

/// A plugin that represents an out-of-process capability provider.
///
/// Without a run function it returns `NotImplemented` for every capability,
/// preserving backward compatibility with callers that only need to reserve a
/// capability id. Provide a run function via [`ProcessPlugin::with_run_fn`] to
/// make it functional.
pub struct ProcessPlugin {
    name: String,
    capabilities: Vec<CapabilityId>,
    run_fn: Option<RunFn>,
}

impl ProcessPlugin {
    /// Creates a new process plugin placeholder that returns `NotImplemented`.
    pub fn new(name: impl Into<String>, capabilities: &[CapabilityId]) -> Self {
        Self {
            name: name.into(),
            capabilities: capabilities.to_vec(),
            run_fn: None,
        }
    }

    /// Creates a process plugin with a custom run function.
    pub fn with_run_fn<F>(name: impl Into<String>, capabilities: &[CapabilityId], run_fn: F) -> Self
    where
        F: Fn(&CapabilityRequest, &mut PluginContext) -> Result<CapabilityResponse, PlatformError>
            + Send
            + Sync
            + 'static,
    {
        Self {
            name: name.into(),
            capabilities: capabilities.to_vec(),
            run_fn: Some(Box::new(run_fn)),
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
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        match &self.run_fn {
            Some(f) => f(request, ctx),
            None => Err(PlatformError::NotImplemented),
        }
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
    ) -> PluginContext<'a> {
        PluginContext { plugins, kits }
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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = test_ctx(&mut plugins, &mut kits);
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
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = test_ctx(&mut plugins, &mut kits);
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
    fn process_plugin_with_run_fn_executes() {
        let p = ProcessPlugin::with_run_fn(
            "proc",
            &["remote".into()],
            |req: &CapabilityRequest, _ctx: &mut PluginContext| {
                Ok(CapabilityResponse(serde_json::json!({
                    "echoed": req.capability.as_str(),
                })))
            },
        );
        assert_eq!(p.name(), "proc");
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = test_ctx(&mut plugins, &mut kits);
        let resp = p
            .run(
                &CapabilityRequest {
                    capability: "remote".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["echoed"], "remote");
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

        let mut kits = KitRegistry::new();
        let mut ctx = test_ctx(&mut reg, &mut kits);
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
        let mut kits = KitRegistry::new();
        let ctx = test_ctx(&mut reg, &mut kits);
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
        let mut kits = KitRegistry::new();
        let mut dummy_plugins = PluginRegistry::new();
        let mut ctx = test_ctx(&mut dummy_plugins, &mut kits);

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
