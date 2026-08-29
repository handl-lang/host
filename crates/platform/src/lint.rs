//! Generic lint operation and `BasicLinter` rule acceptor.
//!
//! This module defines the `Lint` operation, `LintBehavior` trait,
//! and a default `BasicLinter` `RuleAcceptor` plugin. The `BasicLinter`
//! has no knowledge of concrete diagnostic rules; it dispatches to registered
//! `LintBehavior` rules keyed by `AstNode.kind`.

use serde_json::Value;

use crate::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// Lint operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Lint;

impl Operation for Lint {
    type Request = LintRequest;
}

/// Request passed to `Lint` behaviors.
#[derive(Debug, Clone)]
pub struct LintRequest {
    /// The AST node to lint.
    pub ast: AstNode,
    /// Diagnostics emitted by lint behaviors.
    pub diagnostics: Vec<String>,
    /// Resolved linter options.
    pub options: ResolvedOptions,
}

/// Behavior trait for linting a specific `AstNode.kind`.
pub trait LintBehavior: Behavior<Lint> {}

impl Contribution for Box<dyn LintBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Lint` operation.
pub struct BasicLinter {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered linting rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn LintBehavior>>,
    /// Option schemas contributed for the linter.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicLinter {
    /// Create a new `BasicLinter` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("lint")],
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicLinter {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicLinter {
    fn id(&self) -> &str {
        "lint.basic"
    }

    fn name(&self) -> &str {
        "Basic linter"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "lint").then(|| {
            serde_json::json!({
                "command": "lint",
                "description": "Lint a Handl module"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "lint" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("lint");

        let mut req = LintRequest {
            ast: AstNode {
                kind: kind.to_string(),
                span: None,
                payload: request.payload.clone(),
            },
            diagnostics: Vec::new(),
            options: ResolvedOptions::new(),
        };

        for rule in &self.rules {
            if rule.kind() == kind {
                rule.apply(&mut req)?;
            }
        }

        Ok(CapabilityResponse(
            serde_json::json!({"diagnostics": req.diagnostics}),
        ))
    }
}

impl RuleAcceptor for BasicLinter {
    type Rule = Box<dyn LintBehavior>;

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
        let plugin = BasicLinter::new();
        let meta = plugin.metadata(&CapabilityId::from("lint")).unwrap();
        assert_eq!(meta["command"], "lint");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_diagnostics_when_rule_matches() {
        struct Warn;

        impl Behavior<Lint> for Warn {
            fn kind(&self) -> &str {
                "unused"
            }

            fn apply(&self, op: &mut LintRequest) -> Result<(), PlatformError> {
                op.diagnostics.push("unused variable".to_string());
                Ok(())
            }
        }

        impl LintBehavior for Warn {}

        let mut linter = BasicLinter::new();
        linter.accept(Box::new(Warn)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = linter
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::json!({"kind": "unused"}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            resp.0,
            serde_json::json!({"diagnostics": ["unused variable"]})
        );
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicLinter::new();
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
