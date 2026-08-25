use handl_lexer_canonical::CanonicalLexer;
use handl_parser::Parser;
use handl_parser_canonical::CanonicalParser;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_tooling::{ArgSpec, CommandProvider, CommandSpec};
use handl_vm_backend_rust_interpreter::{CanonicalContext, Vm};
use handl_vm_host_eval::EvalHost;

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
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if !self.capabilities.contains(&request.capability) {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        let expr_owned = if let Some(expr) = request
            .payload
            .get("expr")
            .and_then(|v| v.as_str())
        {
            expr.to_string()
        } else if let Some(args) = request.payload.get("args").and_then(|v| v.as_array()) {
            if args.is_empty() {
                return Err(PlatformError::InvalidMetadata(
                    r#"missing "expr" or non-empty "args" in request payload"#.to_string(),
                ));
            }
            args.iter()
                .map(|v| v.as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            return Err(PlatformError::InvalidMetadata(
                r#"missing "expr" or "args" in request payload"#.to_string(),
            ));
        };
        let expr = expr_owned.as_str();
        let lexer = CanonicalLexer::default();
        let parser = CanonicalParser;
        let parsed = parser
            .parse_text(expr, &lexer)
            .map_err(|e| PlatformError::Plugin(format!("parse error: {e}")))?;
        let host = EvalHost::default();
        let vm = Vm::new(host);
        let mut ctx = CanonicalContext::new();
        let result = vm
            .eval(&parsed, &mut ctx)
            .map_err(|e| PlatformError::Plugin(format!("eval error: {e}")))?;
        Ok(CapabilityResponse(result.into_inner()))
    }
}

impl CommandProvider for EvalPlugin {
    fn commands(&self) -> Vec<CommandSpec> {
        vec![CommandSpec::new("eval", "Evaluate a Handl expression")
            .arg(ArgSpec::new("expr").description("Expression to evaluate").multiple())]
    }

    fn run(
        &self,
        _command: &str,
        args: &[String],
        host: &handl_tooling::ToolingHost,
    ) -> Result<serde_json::Value, PlatformError> {
        if args.is_empty() {
            return Err(PlatformError::InvalidMetadata(
                "eval requires an expression".into(),
            ));
        }
        let source = args.join(" ");
        let kit = host.kit();
        let output = kit.parse_module_recovered(&source);
        if output
            .diagnostics
            .iter()
            .any(|d| matches!(d.level, handl_expr::DiagnosticLevel::Error))
        {
            for d in &output.diagnostics {
                eprintln!("{}: {}", diagnostic_label(d.level), d.message);
            }
            return Err(PlatformError::Plugin("parse error".into()));
        }
        let result = kit.eval(&output.stmts)?;
        let values: Vec<serde_json::Value> =
            result.values.into_iter().map(|v| v.into_inner()).collect();
        Ok(serde_json::Value::Array(values))
    }
}

fn diagnostic_label(level: handl_expr::DiagnosticLevel) -> &'static str {
    match level {
        handl_expr::DiagnosticLevel::Error => "error",
        handl_expr::DiagnosticLevel::Warning => "warning",
        handl_expr::DiagnosticLevel::Info => "info",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{ExtensionRegistry, KitRegistry, PluginRegistry};

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
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
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
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
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
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
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
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
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
        let (mut plugins, mut kits, mut extensions) = (
            PluginRegistry::new(),
            KitRegistry::new(),
            ExtensionRegistry::new(),
        );
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
            extensions: &mut extensions,
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
