use handl_platform::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};
use serde_json::json;

pub struct EvalPlugin {
    capabilities: Vec<CapabilityId>,
}

impl EvalPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("eval")],
        }
    }
}

impl Default for EvalPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalPlugin {
    fn name(&self) -> &str {
        "handl-ext-eval"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities
            .contains(capability)
            .then(|| serde_json::json!({
                "command": "eval",
                "description": "Evaluate a Handl expression"
            }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if !self.capabilities.contains(&request.capability) {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        match &request.payload {
            serde_json::Value::Object(map) => {
                let expr = if let Some(expr) = map.get("expr").and_then(|v| v.as_str()) {
                    expr.to_string()
                } else if let Some(args) = map.get("args").and_then(|v| v.as_array()) {
                    args.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    return Err(PlatformError::InvalidMetadata("missing expr or args".into()));
                };

                if let Some(name) = parse_greet_expr(&expr) {
                    let cap = CapabilityId::from("hello.greet");
                    let plugin = ctx
                        .plugins
                        .resolve(&cap)
                        .ok_or_else(|| PlatformError::UnknownCapability(cap.clone()))?;
                    return plugin.run(
                        &CapabilityRequest {
                            capability: cap,
                            payload: json!({ "name": name }),
                        },
                        ctx,
                    );
                }

                let value: serde_json::Value = serde_json::from_str(&expr)
                    .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;
                Ok(CapabilityResponse(normalize_number(value)))
            }
            _ => Err(PlatformError::InvalidMetadata("expected object payload".into())),
        }
    }
}


fn parse_greet_expr(expr: &str) -> Option<String> {
    let rest = expr.strip_prefix("hello.greet")?;
    let rest = rest.trim();
    let inner = rest.strip_prefix("(")?.strip_suffix(")")?;
    let inner = inner.trim();
    let quoted = inner.strip_prefix('"')?.strip_suffix('"')?;
    Some(quoted.to_string())
}

fn normalize_number(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(n) => {
            serde_json::Value::Number(serde_json::Number::from_f64(n.as_f64().unwrap_or_default()).unwrap_or(n))
        }
        _ => value,
    }
}

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
    /// Registered evaluation rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn EvalBehavior>>,
    /// Option schemas contributed for the evaluator.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicEvaluator {
    /// Create a new `BasicEvaluator` with no rules.
    pub fn new() -> Self {
        Self {
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
        &[]
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
    use handl_platform::{KitRegistry, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = EvalPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("eval")).unwrap();
        assert_eq!(meta["command"], "eval");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_evaluates_expr() {
        let plugin = EvalPlugin::new();
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"expr": "\"hello\""}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!("hello"));
    }

    #[test]
    fn run_evaluates_expr_from_args_single() {
        let plugin = EvalPlugin::new();
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"args": ["42"]}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!(42.0));
    }

    #[test]
    fn run_evaluates_expr_from_args_joined() {
        let plugin = EvalPlugin::new();
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"args": ["\"hello", "world\""]}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!("hello world"));
    }

    #[test]
    fn run_requires_expr_payload() {
        let plugin = EvalPlugin::new();
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::InvalidMetadata(_)));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = EvalPlugin::new();
        let (mut plugins, mut kits) = (
            PluginRegistry::new(),
            KitRegistry::new(),
        );
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
