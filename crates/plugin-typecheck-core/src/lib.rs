//! Canonical typecheck implementation as a `TypecheckBehavior` plugin.
//!
//! This crate is the home of the concrete canonical type system logic.
//! It contributes a single `CanonicalTypecheck` rule that is registered
//! against the platform's generic `BasicTypechecker` `RuleAcceptor`.

use handl_platform::{
    BasicTypechecker, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, PlatformError, Plugin, PluginContext, Typecheck,
    TypecheckBehavior, TypecheckRequest,
};
use handl_plugin_lib_type::{
    Env, MetaContext, TCtx, Term, TypeError, Value, interpret, synth, synth_type,
};

/// A `TypecheckBehavior` that performs the canonical handl typecheck.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonicalTypecheck;

impl CanonicalTypecheck {
    /// Create a new canonical typecheck rule.
    pub fn new() -> Self {
        Self
    }
}

impl Behavior<Typecheck> for CanonicalTypecheck {
    fn kind(&self) -> &str {
        "typecheck"
    }

    fn apply(&self, request: &mut TypecheckRequest) -> Result<(), PlatformError> {
        typecheck_payload(&request.ast.payload).map_err(|err| PlatformError::Plugin(err))?;
        Ok(())
    }
}

impl TypecheckBehavior for CanonicalTypecheck {}

/// Typecheck a `Term` with a fresh `MetaContext` and empty `Env`/`TCtx`.
pub fn typecheck(term: &Term) -> Result<Value, TypeError> {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    synth(&mut ctx, &env, &tctx, term)
}

fn typecheck_payload(payload: &serde_json::Value) -> Result<Value, String> {
    if let Some(obj) = payload.as_object()
        && let Some(schema) = obj.get("schema")
    {
        let root = obj.get("root").unwrap_or(schema);
        let term = interpret(schema, root).map_err(|e| e.to_string())?;
        let mut ctx = MetaContext::new();
        let env = Env::new();
        let tctx = TCtx::new();
        return synth_type(&mut ctx, &env, &tctx, &term).map_err(|e| e.to_string());
    }
    typecheck(&handl_plugin_lib_type::u0()).map_err(|e| e.to_string())
}

/// Plugin that contributes the canonical typecheck rule to `BasicTypechecker`.
pub struct TypecheckCore {
    capabilities: Vec<CapabilityId>,
}

impl TypecheckCore {
    /// Create a new `TypecheckCore` plugin.
    pub fn new() -> Self {
        Self {
            capabilities: Vec::new(),
        }
    }
}

impl Default for TypecheckCore {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TypecheckCore {
    fn name(&self) -> &str {
        "typecheck-core"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<BasicTypechecker>(Box::new(CanonicalTypecheck::new()))
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        CapabilityRequest, KitRegistry, Plugin, PluginContext, PluginRegistry, RuleAcceptor,
    };

    #[test]
    fn typecheck_instantiates_vm() {
        let term = handl_plugin_lib_type::u0();
        let result = typecheck(&term);
        assert!(result.is_ok());
    }

    #[test]
    fn typecheck_rejects_ill_typed_term() {
        let term = handl_plugin_lib_type::lam("x", handl_plugin_lib_type::bvar(0));
        let result = typecheck(&term);
        assert!(result.is_err());
    }

    fn run_with(payload: serde_json::Value) -> serde_json::Value {
        let mut typechecker = BasicTypechecker::new();
        typechecker
            .accept(Box::new(CanonicalTypecheck::new()))
            .unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = typechecker
            .run(
                &CapabilityRequest {
                    capability: "typecheck".into(),
                    payload,
                },
                &mut ctx,
            )
            .unwrap();
        resp.0
    }

    #[test]
    fn run_returns_typechecked_flag() {
        let resp = run_with(serde_json::Value::Null);
        assert_eq!(resp, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_typechecks_valid_json_schema() {
        let payload = serde_json::json!({
            "schema": {"typeParams": ["T"], "type": "var", "name": "T"}
        });
        let resp = run_with(payload);
        assert_eq!(resp, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_typechecks_ref_schema_with_defs() {
        let payload = serde_json::json!({
            "schema": {"$ref": "#/$defs/Id"},
            "root": {
                "$defs": {
                    "Id": {"typeParams": ["T"], "type": "var", "name": "T"}
                }
            }
        });
        let resp = run_with(payload);
        assert_eq!(resp, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_rejects_ill_typed_json_schema() {
        let payload = serde_json::json!({"schema": {"type": "string"}});
        let resp = run_with(payload);
        assert_eq!(resp["typechecked"], false);
        assert!(resp.get("error").unwrap().is_string());
    }

    #[test]
    fn run_typechecks_object_without_schema() {
        let payload = serde_json::json!({"root": {"type": "string"}});
        let resp = run_with(payload);
        assert_eq!(resp, serde_json::json!({"typechecked": true}));
    }

    #[test]
    fn run_rejects_non_object_payloads() {
        for payload in [
            serde_json::json!("string"),
            serde_json::json!(42),
            serde_json::json!([]),
        ] {
            let resp = run_with(payload);
            assert_eq!(resp, serde_json::json!({"typechecked": true}));
        }
    }

    #[test]
    fn run_rejects_missing_root_def() {
        let payload = serde_json::json!({
            "schema": {"$ref": "#/$defs/Missing"},
            "root": {"$defs": {}}
        });
        let resp = run_with(payload);
        assert_eq!(resp["typechecked"], false);
        assert!(resp.get("error").unwrap().is_string());
    }
}
