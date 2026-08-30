//! # handl-plugin-eval-object
//!
//! `EvalBehavior` rule for object literal expressions (kind `"object"`).
//!
//! Ported from `ObjectHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"entries": [{"key": "k", "value": <ast>}, ...]}`;
//! the rule evaluates each value via `ctx.eval()` and collects the
//! results into a `serde_json::Value::Object`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for object literal expressions.
///
/// `AstNode.payload` is expected to contain
/// `{"entries": [{"key": "k", "value": <ast>}, ...]}`.
/// Each value AST is evaluated via `req.ctx.eval()` and the results are
/// collected into a `Value::Object`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ObjectRule;

impl Behavior<Eval> for ObjectRule {
    fn kind(&self) -> &str {
        "object"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let entries = req
            .ast
            .payload
            .get("entries")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EvalError::TypeMismatch("object: missing or non-array \"entries\" field".into())
            })?;

        let mut map = serde_json::Map::with_capacity(entries.len());
        for entry in entries {
            let key = entry
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    EvalError::TypeMismatch("object: entry missing or non-string \"key\"".into())
                })?;
            let value_ast = entry
                .get("value")
                .ok_or_else(|| {
                    EvalError::TypeMismatch("object: entry missing \"value\"".into())
                })?;
            let ast = ast_from_value(value_ast)?;
            let value = req.ctx.eval(&ast)?;
            map.insert(key.to_string(), value);
        }

        req.result = Some(Value::Object(map));
        Ok(())
    }
}

impl EvalBehavior for ObjectRule {}

/// Reconstruct an [`AstNode`] from a JSON value (serialized `AstNode`).
fn ast_from_value(value: &Value) -> Result<AstNode, EvalError> {
    serde_json::from_value::<AstNode>(value.clone()).map_err(|e| {
        EvalError::TypeMismatch(format!("object: failed to deserialize child AST: {e}"))
    })
}

/// Registers the `object` rule with the [`BasicEvaluator`].
pub struct EvalObjectPlugin {
    registered: AtomicBool,
}

impl EvalObjectPlugin {
    /// Create a new `EvalObjectPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalObjectPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalObjectPlugin {
    fn id(&self) -> &str {
        "eval.object"
    }

    fn name(&self) -> &str {
        "eval-object"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(ObjectRule) as Arc<dyn EvalBehavior>)?;
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
            .accept(Arc::new(ObjectRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    #[test]
    fn object_empty() {
        let evaluator = make_evaluator();
        let payload = json!({ "kind": "object", "entries": [] });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!({}));
    }

    #[test]
    fn object_with_values() {
        let evaluator = make_evaluator();
        let payload = json!({
            "kind": "object",
            "entries": [
                { "key": "a", "value": { "kind": "value", "span": null, "payload": { "value": 1 } } },
                { "key": "b", "value": { "kind": "value", "span": null, "payload": { "value": "two" } } }
            ]
        });
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!({ "a": 1, "b": "two" }));
    }

    #[test]
    fn object_missing_entries() {
        let evaluator = make_evaluator();
        let payload = json!({ "kind": "object" });
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("entries"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalObjectPlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
