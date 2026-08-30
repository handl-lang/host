//! # handl-plugin-eval-array
//!
//! `EvalBehavior` rule for array literal expressions (kind `"array"`).
//!
//! Ported from `ArrayHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"elements": [<ast>, ...]}`; the rule evaluates
//! each element via `ctx.eval()` and collects the results into a
//! `serde_json::Value::Array`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for array literal expressions.
///
/// `AstNode.payload` is expected to contain `{"elements": [<ast>, ...]}`.
/// Each element AST is evaluated via `req.ctx.eval()` and the results are
/// collected into a `Value::Array`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArrayRule;

impl Behavior<Eval> for ArrayRule {
    fn kind(&self) -> &str {
        "array"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let elements = req
            .ast
            .payload
            .get("elements")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EvalError::TypeMismatch("array: missing or non-array \"elements\" field".into())
            })?;

        let mut values = Vec::with_capacity(elements.len());
        for elem in elements {
            let elem_ast = ast_from_value(elem)?;
            let value = req.ctx.eval(&elem_ast)?;
            values.push(value);
        }

        req.result = Some(Value::Array(values));
        Ok(())
    }
}

impl EvalBehavior for ArrayRule {}

/// Reconstruct an [`AstNode`] from a JSON value (serialized `AstNode`).
fn ast_from_value(value: &Value) -> Result<AstNode, EvalError> {
    serde_json::from_value::<AstNode>(value.clone()).map_err(|e| {
        EvalError::TypeMismatch(format!("array: failed to deserialize child AST: {e}"))
    })
}

/// Registers the `array` rule with the [`BasicEvaluator`].
pub struct EvalArrayPlugin {
    registered: AtomicBool,
}

impl EvalArrayPlugin {
    /// Create a new `EvalArrayPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalArrayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalArrayPlugin {
    fn id(&self) -> &str {
        "eval.array"
    }

    fn name(&self) -> &str {
        "eval-array"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(ArrayRule) as Arc<dyn EvalBehavior>)?;
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

    /// A test-helper `Behavior<Eval>` rule (kind `"value"`) that returns
    /// the literal from `payload.value`.
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
            .accept(Arc::new(ArrayRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    #[test]
    fn array_empty() {
        let evaluator = make_evaluator();
        let payload = json!({ "kind": "array", "elements": [] });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn array_of_values() {
        let evaluator = make_evaluator();
        let payload = json!({
            "kind": "array",
            "elements": [
                { "kind": "value", "span": null, "payload": { "value": 1 } },
                { "kind": "value", "span": null, "payload": { "value": 2 } },
                { "kind": "value", "span": null, "payload": { "value": 3 } }
            ]
        });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!([1, 2, 3]));
    }

    #[test]
    fn array_mixed_types() {
        let evaluator = make_evaluator();
        let payload = json!({
            "kind": "array",
            "elements": [
                { "kind": "value", "span": null, "payload": { "value": 42 } },
                { "kind": "value", "span": null, "payload": { "value": "hello" } },
                { "kind": "value", "span": null, "payload": { "value": true } },
                { "kind": "value", "span": null, "payload": { "value": null } }
            ]
        });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!([42, "hello", true, null]));
    }

    #[test]
    fn array_missing_elements() {
        let evaluator = make_evaluator();
        let payload = json!({ "kind": "array" });
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("elements"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalArrayPlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
