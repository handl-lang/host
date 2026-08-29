//! Generic build operation and `BasicBuilder` rule acceptor.
//!
//! This module defines the `Build` operation, `BuildBehavior` trait,
//! and a default `BasicBuilder` `RuleAcceptor` plugin. The `BasicBuilder`
//! has no knowledge of concrete build targets; it dispatches to registered
//! `BuildBehavior` rules keyed by `AstNode.kind`.

use serde_json::Value;

use crate::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// Build operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Build;

impl Operation for Build {
    type Request = BuildRequest;
}

/// Request passed to `Build` behaviors.
#[derive(Debug, Clone)]
pub struct BuildRequest {
    /// The AST node describing the build target.
    pub ast: AstNode,
    /// Whether the build succeeded.
    pub built: bool,
    /// Resolved builder options.
    pub options: ResolvedOptions,
}

/// Behavior trait for building a specific `AstNode.kind`.
pub trait BuildBehavior: Behavior<Build> {}

impl Contribution for Box<dyn BuildBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Build` operation.
pub struct BasicBuilder {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered build rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn BuildBehavior>>,
    /// Option schemas contributed for the builder.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicBuilder {
    /// Create a new `BasicBuilder` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("build")],
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicBuilder {
    fn id(&self) -> &str {
        "build.basic"
    }

    fn name(&self) -> &str {
        "Basic builder"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "build").then(|| {
            serde_json::json!({
                "command": "build",
                "description": "Build a Handl project"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "build" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("build");

        let mut req = BuildRequest {
            ast: AstNode {
                kind: kind.to_string(),
                span: None,
                payload: request.payload.clone(),
            },
            built: true,
            options: ResolvedOptions::new(),
        };

        for rule in &self.rules {
            if rule.kind() == kind {
                rule.apply(&mut req)?;
            }
        }

        Ok(CapabilityResponse(serde_json::json!({"built": req.built})))
    }
}

impl RuleAcceptor for BasicBuilder {
    type Rule = Box<dyn BuildBehavior>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KitRegistry, PluginContext, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = BasicBuilder::new();
        let meta = plugin.metadata(&CapabilityId::from("build")).unwrap();
        assert_eq!(meta["command"], "build");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_built_flag_when_rule_matches() {
        struct Succeed;

        impl Behavior<Build> for Succeed {
            fn kind(&self) -> &str {
                "build"
            }

            fn apply(&self, op: &mut BuildRequest) -> Result<(), PlatformError> {
                op.built = true;
                Ok(())
            }
        }

        impl BuildBehavior for Succeed {}

        let mut builder = BasicBuilder::new();
        builder.accept(Box::new(Succeed)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = builder
            .run(
                &CapabilityRequest {
                    capability: "build".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"built": true}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicBuilder::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
