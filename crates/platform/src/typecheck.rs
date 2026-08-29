//! Generic typecheck operation and `BasicTypechecker` rule acceptor.
//!
//! This module defines the `Typecheck` operation, `TypecheckBehavior` trait,
//! and a default `BasicTypechecker` `RuleAcceptor` plugin.  The `BasicTypechecker`
//! has no knowledge of concrete value types; it dispatches to registered
//! `TypecheckBehavior` rules keyed by `AstNode.kind`.

use serde_json::Value;

use crate::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// Typecheck operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Typecheck;

impl Operation for Typecheck {
    type Request = TypecheckRequest;
}

/// Request passed to `Typecheck` behaviors.
#[derive(Debug, Clone)]
pub struct TypecheckRequest {
    /// The AST node to typecheck.
    pub ast: AstNode,
    /// Resolved typechecker options.
    pub options: ResolvedOptions,
}

/// Behavior trait for typechecking a specific `AstNode.kind`.
pub trait TypecheckBehavior: Behavior<Typecheck> {}

impl Contribution for Box<dyn TypecheckBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Typecheck` operation.
pub struct BasicTypechecker {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered typechecking rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn TypecheckBehavior>>,
    /// Option schemas contributed for the typechecker.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicTypechecker {
    /// Create a new `BasicTypechecker` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("typecheck")],
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicTypechecker {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicTypechecker {
    fn id(&self) -> &str {
        "typecheck.basic"
    }

    fn name(&self) -> &str {
        "Basic typechecker"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "typecheck").then(|| {
            serde_json::json!({
                "command": "typecheck",
                "description": "Typecheck a Handl module"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "typecheck" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("typecheck");

        let mut req = TypecheckRequest {
            ast: AstNode {
                kind: kind.to_string(),
                span: None,
                payload: request.payload.clone(),
            },
            options: ResolvedOptions::new(),
        };

        for rule in &self.rules {
            if rule.kind() == kind {
                if let Err(err) = rule.apply(&mut req) {
                    return Ok(CapabilityResponse(
                        serde_json::json!({"typechecked": false, "error": err.to_string()}),
                    ));
                }
            }
        }

        Ok(CapabilityResponse(serde_json::json!({"typechecked": true})))
    }
}

impl RuleAcceptor for BasicTypechecker {
    type Rule = Box<dyn TypecheckBehavior>;

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
        let plugin = BasicTypechecker::new();
        let meta = plugin.metadata(&CapabilityId::from("typecheck")).unwrap();
        assert_eq!(meta["command"], "typecheck");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_typechecked_flag_when_no_rules() {
        let plugin = BasicTypechecker::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicTypechecker::new();
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
