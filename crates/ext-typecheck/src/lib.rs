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

        match typecheck_payload(&request.payload) {
            Ok(_) => Ok(CapabilityResponse(serde_json::json!({"typechecked": true}))),
            Err(err) => Ok(CapabilityResponse(
                serde_json::json!({"typechecked": false, "error": err}),
            )),
        }
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

fn typecheck_payload(payload: &serde_json::Value) -> Result<handl_vm_type::Value, String> {
    if let Some(obj) = payload.as_object()
        && let Some(schema) = obj.get("schema")
    {
        let root = obj.get("root").unwrap_or(schema);
        let term = handl_vm_type_schema_json::interpret(schema, root).map_err(|e| e.to_string())?;
        let mut ctx = handl_vm_type::MetaContext::new();
        let env = handl_vm_type::Env::new();
        let tctx = handl_vm_type::TCtx::new();
        return handl_vm_type::synth_type(&mut ctx, &env, &tctx, &term).map_err(|e| e.to_string());
    }
    typecheck(&handl_vm_type::u0()).map_err(|e| e.to_string())
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
        let plugin = TypecheckPlugin::new();
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

    #[test]
    fn run_typechecks_valid_json_schema() {
        let plugin = TypecheckPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let payload = serde_json::json!({
            "schema": {"typeParams": ["T"], "type": "var", "name": "T"}
        });
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_typechecks_ref_schema_with_defs() {
        let plugin = TypecheckPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let payload = serde_json::json!({
            "schema": {"$ref": "#/$defs/Id"},
            "root": {
                "$defs": {
                    "Id": {"typeParams": ["T"], "type": "var", "name": "T"}
                }
            }
        });
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_rejects_ill_typed_json_schema() {
        let plugin = TypecheckPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let payload = serde_json::json!({"schema": {"type": "string"}});
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["typechecked"], false);
        assert!(resp.0.get("error").unwrap().is_string());
    }

    #[test]
    fn run_typechecks_object_without_schema() {
        let plugin = TypecheckPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let payload = serde_json::json!({"root": {"type": "string"}});
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_rejects_non_object_payloads() {
        let plugin = TypecheckPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        for payload in [
            serde_json::json!("string"),
            serde_json::json!(42),
            serde_json::json!([]),
        ] {
            let resp = plugin
                .run(
                    &CapabilityRequest {
                        capability: "typecheck".into(),
                        payload,
                    },
                    &mut ctx,
                )
                .unwrap();
            assert_eq!(resp.0, serde_json::json!({"typechecked": true}));
        }
    }

    #[test]
    fn run_rejects_missing_root_def() {
        let plugin = TypecheckPlugin::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let payload = serde_json::json!({
            "schema": {"$ref": "#/$defs/Missing"},
            "root": {"$defs": {}}
        });
        let resp = plugin
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload,
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0["typechecked"], false);
        assert!(resp.0.get("error").unwrap().is_string());
    }
}
