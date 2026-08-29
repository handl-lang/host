//! Generic compile operation and `BasicCompiler` rule acceptor.
//!
//! This module defines the `Compile` operation, `CompileBehavior` trait,
//! and a default `BasicCompiler` `RuleAcceptor` plugin. The `BasicCompiler`
//! has no knowledge of concrete compilation targets; it dispatches to registered
//! `CompileBehavior` rules keyed by `AstNode.kind`.

use serde_json::Value;

use crate::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

/// Compile operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Compile;

impl Operation for Compile {
    type Request = CompileRequest;
}

/// Request passed to `Compile` behaviors.
#[derive(Debug, Clone)]
pub struct CompileRequest {
    /// The AST node describing the compilation target.
    pub ast: AstNode,
    /// Whether the compilation succeeded.
    pub compiled: bool,
    /// Resolved compiler options.
    pub options: ResolvedOptions,
}

/// Behavior trait for compiling a specific `AstNode.kind`.
pub trait CompileBehavior: Behavior<Compile> {}

impl Contribution for Box<dyn CompileBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Compile` operation.
pub struct BasicCompiler {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered compile rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn CompileBehavior>>,
    /// Option schemas contributed for the compiler.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicCompiler {
    /// Create a new `BasicCompiler` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("compile")],
            rules: Vec::new(),
            option_schemas: Vec::new(),
        }
    }
}

impl Default for BasicCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicCompiler {
    fn id(&self) -> &str {
        "compile.basic"
    }

    fn name(&self) -> &str {
        "Basic compiler"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "compile").then(|| {
            serde_json::json!({
                "command": "compile",
                "description": "Compile a Handl module"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "compile" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("compile");

        let mut req = CompileRequest {
            ast: AstNode {
                kind: kind.to_string(),
                span: None,
                payload: request.payload.clone(),
            },
            compiled: true,
            options: ResolvedOptions::new(),
        };

        for rule in &self.rules {
            if rule.kind() == kind {
                rule.apply(&mut req)?;
            }
        }

        Ok(CapabilityResponse(
            serde_json::json!({"compiled": req.compiled}),
        ))
    }
}

impl RuleAcceptor for BasicCompiler {
    type Rule = Box<dyn CompileBehavior>;

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
        let plugin = BasicCompiler::new();
        let meta = plugin.metadata(&CapabilityId::from("compile")).unwrap();
        assert_eq!(meta["command"], "compile");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_compiled_flag_when_rule_matches() {
        struct Succeed;

        impl Behavior<Compile> for Succeed {
            fn kind(&self) -> &str {
                "compile"
            }

            fn apply(&self, op: &mut CompileRequest) -> Result<(), PlatformError> {
                op.compiled = true;
                Ok(())
            }
        }

        impl CompileBehavior for Succeed {}

        let mut compiler = BasicCompiler::new();
        compiler.accept(Box::new(Succeed)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = compiler
            .run(
                &CapabilityRequest {
                    capability: "compile".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"compiled": true}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicCompiler::new();
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
