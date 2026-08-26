use handl_platform::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

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
    /// Registered typechecking rules, keyed by `AstNode.kind`.
    pub rules: Vec<Box<dyn TypecheckBehavior>>,
    /// Option schemas contributed for the typechecker.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicTypechecker {
    /// Create a new `BasicTypechecker` with no rules.
    pub fn new() -> Self {
        Self {
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
    use handl_platform::{KitRegistry, PluginRegistry};

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
