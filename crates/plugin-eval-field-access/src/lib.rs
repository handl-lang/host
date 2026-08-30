//! # handl-plugin-eval-field-access
//!
//! `EvalBehavior` rule for field access expressions (kind `"field-access"`).
//!
//! Ported from `FieldAccessHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"target": <ast>, "field": "name"}`; the rule
//! evaluates the target, then accesses the named field on the resulting
//! JSON object. If the target is not an object, a type mismatch error is
//! returned.
//!
//! Note: the current grammar (`plugin-grammar-index`) handles dot access
//! via the `"index"` kind (with a string index). This `"field-access"` kind
//! is provided for dialects that prefer a dedicated field-access AST node.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for field access expressions.
///
/// `AstNode.payload` is expected to contain
/// `{"target": <ast>, "field": "name"}`.
/// The target is evaluated via `req.ctx.eval()`. If the result is a JSON
/// object, the named field is retrieved (defaulting to `null` if absent).
/// Otherwise a type mismatch error is returned.
#[derive(Debug, Default, Clone, Copy)]
pub struct FieldAccessRule;

impl Behavior<Eval> for FieldAccessRule {
    fn kind(&self) -> &str {
        "field-access"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let target_value = req.ast.payload.get("target").ok_or_else(|| {
            EvalError::TypeMismatch("field-access: missing \"target\" field".into())
        })?;
        let field = req
            .ast
            .payload
            .get("field")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                EvalError::TypeMismatch("field-access: missing or non-string \"field\"".into())
            })?;

        let target_ast = ast_from_value(target_value)?;
        let target = req.ctx.eval(&target_ast)?;

        match &target {
            Value::Object(map) => {
                let value = map.get(field).cloned().unwrap_or(Value::Null);
                req.result = Some(value);
                Ok(())
            }
            _ => Err(EvalError::TypeMismatch(format!(
                "field-access: expected object, got {}",
                json_kind(&target)
            ))
            .into()),
        }
    }
}

impl EvalBehavior for FieldAccessRule {}

/// Return a human-readable name for a JSON value type.
fn json_kind(v: &Value) -> &'static str {
    match v {
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
    }
}

/// Reconstruct an [`AstNode`] from a JSON value (serialized `AstNode`).
fn ast_from_value(value: &Value) -> Result<AstNode, EvalError> {
    serde_json::from_value::<AstNode>(value.clone()).map_err(|e| {
        EvalError::TypeMismatch(format!("field-access: failed to deserialize child AST: {e}"))
    })
}

/// Registers the `field-access` rule with the [`BasicEvaluator`].
pub struct EvalFieldAccessPlugin {
    registered: AtomicBool,
}

impl EvalFieldAccessPlugin {
    /// Create a new `EvalFieldAccessPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalFieldAccessPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalFieldAccessPlugin {
    fn id(&self) -> &str {
        "eval.field-access"
    }

    fn name(&self) -> &str {
        "eval-field-access"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if !self.registered.load(Ordering::SeqCst) {
            self.registered.store(true, Ordering::SeqCst);
            ctx.contribute::<BasicEvaluator>(Arc::new(FieldAccessRule) as Arc<dyn EvalBehavior>)?;
        }
        Ok(())
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
        BasicEvaluator, CapabilityId, CapabilityRequest, PluginContext, PluginRegistry,
        KitRegistry, RuleAcceptor,
    };
    use serde_json::json;

    /// A test-helper `Behavior<Eval>` rule (kind `"with_scope"`) that seeds
    /// globals then evaluates a child node.
    struct WithScope;
    impl Behavior<Eval> for WithScope {
        fn kind(&self) -> &str {
            "with_scope"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            if let Some(globals) = req.ast.payload.get("globals").and_then(Value::as_object) {
                for (name, value) in globals {
                    req.ctx.define(name.clone(), value.clone());
                }
            }
            let child = req.ast.payload.get("child").cloned().unwrap_or(Value::Null);
            let child_ast = serde_json::from_value::<AstNode>(child)
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            let result = req.ctx.eval(&child_ast)?;
            req.result = Some(result);
            Ok(())
        }
    }
    impl EvalBehavior for WithScope {}

    /// A test-helper `Behavior<Eval>` rule (kind `"var"`) for variable lookup.
    struct Var;
    impl Behavior<Eval> for Var {
        fn kind(&self) -> &str {
            "var"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            let name = req
                .ast
                .payload
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| EvalError::TypeMismatch("var: missing \"name\"".into()))?;
            let value = req.ctx.get(name).map_err(PlatformError::from)?;
            req.result = Some(value);
            Ok(())
        }
    }
    impl EvalBehavior for Var {}

    fn run_eval(evaluator: &BasicEvaluator, payload: Value) -> Result<Value, PlatformError> {
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = evaluator.run(
            &CapabilityRequest {
                capability: CapabilityId::from("eval"),
                payload,
            },
            &mut pctx,
        )?;
        Ok(resp.0)
    }

    fn make_evaluator() -> BasicEvaluator {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(FieldAccessRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(WithScope) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(Var) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    fn with_scope(globals: Value, child: Value) -> Value {
        json!({ "kind": "with_scope", "globals": globals, "child": child })
    }

    #[test]
    fn field_access_object() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "obj": { "a": 1, "b": 2 } }),
            json!({
                "kind": "field-access",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "obj" } },
                    "field": "b"
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn field_access_missing_field_returns_null() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "obj": { "a": 1 } }),
            json!({
                "kind": "field-access",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "obj" } },
                    "field": "z"
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(null));
    }

    #[test]
    fn field_access_non_object() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "n": 42 }),
            json!({
                "kind": "field-access",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "n" } },
                    "field": "x"
                }
            }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("expected object"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalFieldAccessPlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
