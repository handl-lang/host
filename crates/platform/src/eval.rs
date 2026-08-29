//! Generic evaluation operation and `BasicEvaluator` rule acceptor.
//!
//! This module defines the `Eval` operation, `EvalBehavior` trait,
//! and a default `BasicEvaluator` `RuleAcceptor` plugin. The `BasicEvaluator`
//! has no knowledge of concrete language builtins; it dispatches to registered
//! `EvalBehavior` rules keyed by `AstNode.kind`.

use serde_json::Value;

use crate::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// Eval operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Eval;

impl Operation for Eval {
    type Request = EvalRequest;
}

/// Request passed to `Eval` behaviors.
#[derive(Debug, Clone)]
pub struct EvalRequest {
    /// The AST node to evaluate.
    pub ast: AstNode,
    /// Resolved evaluator options.
    pub options: ResolvedOptions,
    /// Optional result produced by a behavior.
    pub result: Option<Value>,
}

/// Behavior trait for evaluating a specific `AstNode.kind`.
pub trait EvalBehavior: Behavior<Eval> {}

impl Contribution for Box<dyn EvalBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Eval` operation.
pub struct BasicEvaluator {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered evaluation rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn EvalBehavior>>,
    /// Option schemas contributed for the evaluator.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicEvaluator {
    /// Create a new `BasicEvaluator` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("eval")],
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicEvaluator {
    fn id(&self) -> &str {
        "eval.basic"
    }

    fn name(&self) -> &str {
        "Basic evaluator"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "eval").then(|| {
            serde_json::json!({
                "command": "eval",
                "description": "Evaluate a Handl expression"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if !self.capabilities.contains(&request.capability) {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("eval");

        let mut req = EvalRequest {
            ast: AstNode {
                kind: kind.to_string(),
                span: None,
                payload: request.payload.clone(),
            },
            options: ResolvedOptions::new(),
            result: None,
        };

        for rule in &self.rules {
            if rule.kind() == kind {
                rule.apply(&mut req)?;
            }
        }

        let result = req.result.ok_or(PlatformError::NotImplemented)?;
        Ok(CapabilityResponse(result))
    }
}

impl RuleAcceptor for BasicEvaluator {
    type Rule = Box<dyn EvalBehavior>;

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
        let plugin = BasicEvaluator::new();
        let meta = plugin.metadata(&CapabilityId::from("eval")).unwrap();
        assert_eq!(meta["command"], "eval");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_evaluated_result_when_rule_matches() {
        struct Echo;

        impl Behavior<Eval> for Echo {
            fn kind(&self) -> &str {
                "echo"
            }

            fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
                op.result = Some(op.ast.payload.clone());
                Ok(())
            }
        }

        impl EvalBehavior for Echo {}

        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Box::new(Echo)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"kind": "echo", "value": 42}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"kind": "echo", "value": 42}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicEvaluator::new();
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
