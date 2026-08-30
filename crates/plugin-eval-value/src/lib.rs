//! # handl-plugin-eval-value
//!
//! `EvalBehavior` rule for literal value expressions (kind `"value"`).
//!
//! Ported from `ValueHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"value": <json value>}`; the rule extracts the
//! `value` field and returns it directly.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};

/// Evaluation rule for literal value expressions.
///
/// `AstNode.payload` is expected to contain `{"value": <json value>}`.
/// The rule extracts the `value` field and sets `req.result` to it.
#[derive(Debug, Default, Clone, Copy)]
pub struct ValueRule;

impl Behavior<Eval> for ValueRule {
    fn kind(&self) -> &str {
        "value"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let value = req.ast.payload.get("value").cloned().ok_or_else(|| {
            EvalError::TypeMismatch("value: missing \"value\" field".into())
        })?;
        req.result = Some(value);
        Ok(())
    }
}

impl EvalBehavior for ValueRule {}

/// Registers the `value` rule with the [`BasicEvaluator`].
pub struct EvalValuePlugin {
    registered: AtomicBool,
}

impl EvalValuePlugin {
    /// Create a new `EvalValuePlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalValuePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalValuePlugin {
    fn id(&self) -> &str {
        "eval.value"
    }

    fn name(&self) -> &str {
        "eval-value"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(ValueRule) as Arc<dyn EvalBehavior>)?;
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
    use serde_json::{Value, json};

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

    #[test]
    fn value_number() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(ValueRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        let payload = json!({ "kind": "value", "value": 42 });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn value_string() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(ValueRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        let payload = json!({ "kind": "value", "value": "hello" });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn value_bool() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(ValueRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        let payload = json!({ "kind": "value", "value": true });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn value_null() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(ValueRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        let payload = json!({ "kind": "value", "value": null });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(null));
    }

    #[test]
    fn value_missing_field() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(ValueRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        let payload = json!({ "kind": "value" });
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("value"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalValuePlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
