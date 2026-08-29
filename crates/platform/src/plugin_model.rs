//! Foundational composable plugin types.
//!
//! This module defines the base traits and context used by the
//! `handl-platform` composition model: `Contribution`, `RuleAcceptor`,
//! `Operation`, `Behavior`, and `ContributionContext`.

use std::sync::Arc;

use crate::error::PlatformError;
use crate::kit::KitRegistry;
use crate::capability::CapabilityId;
use crate::plugin::{
    CapabilityRequest, CapabilityResponse, Plugin, PluginContext, PluginRegistry,
};

/// A contribution that can be accepted by a [`RuleAcceptor`].
pub trait Contribution: Send + Sync + 'static {
    /// Returns a stable identifier for this contribution.
    fn id(&self) -> &str;
}

/// A plugin that can accept contributions of a specific rule type.
pub trait RuleAcceptor: Plugin {
    /// The type of rule this acceptor receives.
    type Rule: Contribution;

    /// Accept a rule contribution.
    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError>;
}

/// A marker type for a cross-cutting operation.
pub trait Operation: Send + Sync + 'static {
    /// The request type operated on by behaviors for this operation.
    type Request;
}

/// A rule that provides behavior for a specific AST node kind under an operation.
pub trait Behavior<Op: Operation>: Send + Sync + 'static {
    /// The `AstNode.kind` this behavior handles.
    fn kind(&self) -> &str;

    /// Apply this behavior to the operation request.
    fn apply(&self, op: &mut Op::Request) -> Result<(), PlatformError>;
}

/// Context passed to a plugin during the contribution phase.
pub struct ContributionContext<'a> {
    plugins: &'a mut PluginRegistry,
    kits: &'a mut KitRegistry,
}

impl<'a> ContributionContext<'a> {
    /// Create a new contribution context around the given plugin and kit registries.
    pub fn new(plugins: &'a mut PluginRegistry, kits: &'a mut KitRegistry) -> Self {
        Self { plugins, kits }
    }

    /// Returns the plugin that provides the given capability, if any.
    pub fn resolve(&self, capability: &CapabilityId) -> Option<Arc<dyn Plugin + 'static>> {
        self.plugins.resolve(capability)
    }

    /// Runs the requested capability through the providing plugin.
    pub fn run(
        &mut self,
        request: &CapabilityRequest,
    ) -> Result<CapabilityResponse, PlatformError> {
        let plugin = self
            .resolve(&request.capability)
            .ok_or_else(|| PlatformError::UnknownCapability(request.capability.clone()))?;
        let mut ctx = PluginContext {
            plugins: self.plugins,
            kits: self.kits,
        };
        plugin.run(request, &mut ctx)
    }

    /// Register a plugin with the host.
    pub fn register_plugin(&mut self, plugin: impl Plugin + 'static) -> Result<(), PlatformError> {
        self.plugins.register(plugin);
        Ok(())
    }

    /// Contribute a rule to a registered [`RuleAcceptor`] of type `P`.
    pub fn contribute<P: RuleAcceptor>(&mut self, rule: P::Rule) -> Result<(), PlatformError> {
        match self.plugins.get_plugin_mut::<P>() {
            Some(acceptor) => acceptor.accept(rule),
            None => Err(PlatformError::MissingRuleAcceptor(
                std::any::type_name::<P>().to_string(),
            )),
        }
    }

    /// Returns a mutable reference to a registered plugin of the given type, if any.
    pub fn get_plugin_mut<P: Plugin + 'static>(&mut self) -> Option<&mut P> {
        self.plugins.get_plugin_mut::<P>()
    }
}
