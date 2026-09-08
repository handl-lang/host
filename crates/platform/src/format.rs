//! Generic format operation and `BasicFormatter` rule acceptor.
//!
//! This module defines the `Format` operation, `FormatBehavior` trait,
//! and a default `BasicFormatter` `RuleAcceptor` plugin. The `BasicFormatter`
//! has no knowledge of concrete surface syntax; it dispatches to registered
//! `FormatBehavior` rules keyed by `AstNode.kind`.
//!
//! # Role after ADR-0095
//!
//! The canonical formatter is the **staged engine pipeline** in
//! `handl-plugin-engine-fmt` (layout → render → post-process), surfaced by
//! `handl-plugin-ext-fmt` (`format_source`/`check_source`/`format_ast`). Those
//! crates depend on this crate, so `platform` **cannot** dispatch to the engine
//! here (dependency direction). This `Format` capability therefore remains the
//! **generic per-node contribution seam** of ADR-0014/ADR-0029: hosts and kits
//! register `FormatBehavior` rules for individual node kinds. It is not the
//! canonical formatting path — that is `FormatEngine`/`plugin-ext-fmt`. Keeping
//! the two honest (documented) avoids a disconnected facade.

use serde_json::Value;

use crate::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// Format operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Format;

impl Operation for Format {
    type Request = FormatRequest;
}

/// Request passed to `Format` behaviors.
#[derive(Debug, Clone)]
pub struct FormatRequest {
    /// The AST node to format.
    pub ast: AstNode,
    /// Formatted output collected by behaviors.
    pub output: String,
    /// Resolved formatter options.
    pub options: ResolvedOptions,
}

/// Behavior trait for formatting a specific `AstNode.kind`.
pub trait FormatBehavior: Behavior<Format> {}

impl Contribution for Box<dyn FormatBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Format` operation.
pub struct BasicFormatter {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered formatting rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn FormatBehavior>>,
    /// Option schemas contributed for the formatter.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicFormatter {
    /// Create a new `BasicFormatter` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("fmt")],
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicFormatter {
    fn id(&self) -> &str {
        "fmt.basic"
    }

    fn name(&self) -> &str {
        "Basic formatter"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "fmt").then(|| {
            serde_json::json!({
                "command": "fmt",
                "description": "Format a Handl module"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "fmt" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("fmt");

        let mut req = FormatRequest {
            ast: AstNode {
                kind: kind.to_string(),
                span: None,
                payload: request.payload.clone(),
            },
            output: String::new(),
            options: ResolvedOptions::new(),
        };

        for rule in &self.rules {
            if rule.kind() == kind {
                rule.apply(&mut req)?;
            }
        }

        Ok(CapabilityResponse(
            serde_json::json!({"formatted": req.output}),
        ))
    }
}

impl RuleAcceptor for BasicFormatter {
    type Rule = Box<dyn FormatBehavior>;

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
        let plugin = BasicFormatter::new();
        let meta = plugin.metadata(&CapabilityId::from("fmt")).unwrap();
        assert_eq!(meta["command"], "fmt");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_formatted_output_when_rule_matches() {
        struct Echo;

        impl Behavior<Format> for Echo {
            fn kind(&self) -> &str {
                "echo"
            }

            fn apply(&self, op: &mut FormatRequest) -> Result<(), PlatformError> {
                op.output = op.ast.payload.to_string();
                Ok(())
            }
        }

        impl FormatBehavior for Echo {}

        let mut formatter = BasicFormatter::new();
        formatter.accept(Box::new(Echo)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = formatter
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::json!({"kind": "echo", "value": 42}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            resp.0,
            serde_json::json!({"formatted": serde_json::json!({"kind": "echo", "value": 42}).to_string()})
        );
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicFormatter::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
