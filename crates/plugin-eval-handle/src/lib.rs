//! # handl-plugin-eval-handle
//!
//! `EvalBehavior` rule for handle expressions (kind `"handle"`).
//!
//! Ported from `HandleHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"expr": <ast>}`; the rule evaluates the inner
//! expression and returns the result.
//!
//! In the original interpreter, `HandleHandler` used resumable evaluation
//! to intercept effect operations via handler clauses. The current
//! implementation evaluates the inner expression directly. Full
//! effect-handler semantics (operation interception, yielding,
//! resumability) will be layered in when the `EvalError::Yield` variant
//! and handler-clause payload format are finalized (see ADR 0040 —
//! Continuation / yield / resumability).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for handle expressions.
///
/// `AstNode.payload` is expected to contain `{"expr": <ast>}`.
/// The inner expression is evaluated via `req.ctx.eval()` and its result
/// is returned.
#[derive(Debug, Default, Clone, Copy)]
pub struct HandleRule;

impl Behavior<Eval> for HandleRule {
    fn kind(&self) -> &str {
        "handle"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let expr_value = req.ast.payload.get("expr").ok_or_else(|| {
            EvalError::TypeMismatch("handle: missing \"expr\" field".into())
        })?;

        let expr_ast = ast_from_value(expr_value)?;
        let result = req.ctx.eval(&expr_ast)?;
        req.result = Some(result);
        Ok(())
    }
}

impl EvalBehavior for HandleRule {}

/// Reconstruct an [`AstNode`] from a JSON value (serialized `AstNode`).
fn ast_from_value(value: &Value) -> Result<AstNode, EvalError> {
    serde_json::from_value::<AstNode>(value.clone()).map_err(|e| {
        EvalError::TypeMismatch(format!("handle: failed to deserialize child AST: {e}"))
    })
}

/// Registers the `handle` rule with the [`BasicEvaluator`].
pub struct EvalHandlePlugin {
    registered: AtomicBool,
}

impl EvalHandlePlugin {
    /// Create a new `EvalHandlePlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalHandlePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalHandlePlugin {
    fn id(&self) -> &str {
        "eval.handle"
    }

    fn name(&self) -> &str {
        "eval-handle"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(HandleRule) as Arc<dyn EvalBehavior>)?;
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

    /// A test-helper `Behavior<Eval>` rule (kind `"value"`) for literals.
    struct ValueLit;
    impl Behavior<Eval> for ValueLit {
        fn kind(&self) -> &str {
            "value"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            req.result = Some(req.ast.payload.get("value").cloned().unwrap_or(Value::Null));
            Ok(())
        }
    }
    impl EvalBehavior for ValueLit {}

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
            .accept(Arc::new(HandleRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    #[test]
    fn handle_evaluates_inner_expr() {
        let evaluator = make_evaluator();
        let payload = json!({
            "kind": "handle",
            "expr": { "kind": "value", "span": null, "payload": { "value": 42 } }
        });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn handle_evaluates_string() {
        let evaluator = make_evaluator();
        let payload = json!({
            "kind": "handle",
            "expr": { "kind": "value", "span": null, "payload": { "value": "hello" } }
        });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn handle_missing_expr() {
        let evaluator = make_evaluator();
        let payload = json!({ "kind": "handle" });
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("expr"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalHandlePlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
