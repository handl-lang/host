use handl_kit_canonical::CanonicalKit;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_tooling::{read_source_or_stdin, ArgSpec, CommandProvider, CommandSpec};

pub struct TypecheckPlugin {
    capabilities: Vec<CapabilityId>,
}

impl TypecheckPlugin {
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("typecheck")],
        }
    }
}

impl Default for TypecheckPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TypecheckPlugin {
    fn name(&self) -> &str {
        "handl-ext-typecheck"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        self.capabilities.contains(capability).then(|| {
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
        if !self.capabilities.contains(&request.capability) {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let term = handl_vm_type::Term::Universe(handl_vm_type::Level::zero());
        let typechecked = typecheck(&term).is_ok();
        Ok(CapabilityResponse(
            serde_json::json!({"typechecked": typechecked}),
        ))
    }
}

pub fn typecheck(
    term: &handl_vm_type::Term,
) -> Result<handl_vm_type::Value, handl_vm_type::TypeError> {
    let mut ctx = handl_vm_type::MetaContext::new();
    let env = handl_vm_type::Env::new();
    let tctx = handl_vm_type::TCtx::new();
    handl_vm_type::synth(&mut ctx, &env, &tctx, term)
}

impl CommandProvider for TypecheckPlugin {
    fn commands(&self) -> Vec<CommandSpec> {
        vec![CommandSpec::new("typecheck", "Typecheck a Handl source file")
            .arg(ArgSpec::new("file").description("File to typecheck; stdin if omitted"))]
    }

    fn run(
        &self,
        _command: &str,
        args: &[String],
        _host: &handl_tooling::ToolingHost,
    ) -> Result<serde_json::Value, PlatformError> {
        let source = read_source_or_stdin(args.first().map(|s| s.as_str()))?;
        let kit = CanonicalKit::new();
        let parsed = kit.parse_module_recovered(&source);
        let mut has_errors = parsed
            .diagnostics
            .iter()
            .any(|d| matches!(d.level, handl_expr::DiagnosticLevel::Error));

        for d in &parsed.diagnostics {
            eprintln!("{}: {}", diagnostic_label(d.level), d.message);
        }

        match kit.typecheck(&parsed.stmts) {
            Ok(out) => {
                if !out.success {
                    has_errors = true;
                }
            }
            Err(e) => {
                has_errors = true;
                eprintln!("error: {e}");
            }
        }

        if has_errors {
            return Err(PlatformError::Plugin("typecheck failed".into()));
        }
        Ok(serde_json::json!({"typechecked": true}))
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
        let plugin = TypecheckPlugin::new();
        let meta = plugin.metadata(&CapabilityId::from("typecheck")).unwrap();
        assert_eq!(meta["command"], "typecheck");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_typechecked_flag() {
        let plugin = TypecheckPlugin::new();
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
        let plugin = TypecheckPlugin::new();
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

    #[test]
    fn typecheck_instantiates_vm() {
        let term = handl_vm_type::u0();
        let result = typecheck(&term);
        assert!(result.is_ok());
    }

    #[test]
    fn typecheck_rejects_ill_typed_term() {
        let term = handl_vm_type::lam("x", handl_vm_type::bvar(0));
        let result = typecheck(&term);
        assert!(result.is_err());
    }
}
