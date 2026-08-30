//! # handl-plugin-eval-index
//!
//! `EvalBehavior` rule for index access expressions (kind `"index"`).
//!
//! Ported from `IndexHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"target": <ast>, "index": <ast>}`; the rule
//! evaluates both the target and index, then indexes into the resulting
//! array, object, or string.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for index access expressions.
///
/// `AstNode.payload` is expected to contain
/// `{"target": <ast>, "index": <ast>}`.
/// Both the target and index are evaluated via `req.ctx.eval()`. The
/// target must be an array, object, or string. Array indexing uses
/// numeric indices; object indexing uses string keys; string indexing
/// uses numeric character offsets.
#[derive(Debug, Default, Clone, Copy)]
pub struct IndexRule;

impl Behavior<Eval> for IndexRule {
    fn kind(&self) -> &str {
        "index"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let target_value = req.ast.payload.get("target").ok_or_else(|| {
            EvalError::TypeMismatch("index: missing \"target\" field".into())
        })?;
        let index_value = req.ast.payload.get("index").ok_or_else(|| {
            EvalError::TypeMismatch("index: missing \"index\" field".into())
        })?;

        let target_ast = ast_from_value(target_value)?;
        let target = req.ctx.eval(&target_ast)?;

        let index_ast = ast_from_value(index_value)?;
        let index = req.ctx.eval(&index_ast)?;

        let result = index_into(&target, &index)?;
        req.result = Some(result);
        Ok(())
    }
}

impl EvalBehavior for IndexRule {}

/// Index into a JSON value (array, object, or string).
fn index_into(target: &Value, index: &Value) -> Result<Value, EvalError> {
    match target {
        Value::Array(arr) => {
            let idx = index
                .as_u64()
                .ok_or_else(|| {
                    EvalError::TypeMismatch("index: array index must be a non-negative integer".into())
                })?;
            arr.get(idx as usize)
                .cloned()
                .ok_or_else(|| EvalError::TypeMismatch(format!("index: array index {idx} out of bounds")))
        }
        Value::Object(obj) => {
            let key = index
                .as_str()
                .ok_or_else(|| {
                    EvalError::TypeMismatch("index: object index must be a string".into())
                })?;
            Ok(obj.get(key).cloned().unwrap_or(Value::Null))
        }
        Value::String(s) => {
            let idx = index
                .as_u64()
                .ok_or_else(|| {
                    EvalError::TypeMismatch("index: string index must be a non-negative integer".into())
                })?;
            s.chars()
                .nth(idx as usize)
                .map(|c| Value::String(c.to_string()))
                .ok_or_else(|| {
                    EvalError::TypeMismatch(format!("index: string index {idx} out of bounds"))
                })
        }
        _ => Err(EvalError::TypeMismatch(format!(
            "index: cannot index into {}",
            json_kind(target)
        ))),
    }
}

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
        EvalError::TypeMismatch(format!("index: failed to deserialize child AST: {e}"))
    })
}

/// Registers the `index` rule with the [`BasicEvaluator`].
pub struct EvalIndexPlugin {
    registered: AtomicBool,
}

impl EvalIndexPlugin {
    /// Create a new `EvalIndexPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalIndexPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalIndexPlugin {
    fn id(&self) -> &str {
        "eval.index"
    }

    fn name(&self) -> &str {
        "eval-index"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(IndexRule) as Arc<dyn EvalBehavior>)?;
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
                .ok_or_else(|| {
                    EvalError::TypeMismatch("var: missing \"name\"".into())
                })?;
            let value = req.ctx.get(name).map_err(PlatformError::from)?;
            req.result = Some(value);
            Ok(())
        }
    }
    impl EvalBehavior for Var {}

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
            .accept(Arc::new(IndexRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(WithScope) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(Var) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    fn with_scope(globals: Value, child: Value) -> Value {
        json!({ "kind": "with_scope", "globals": globals, "child": child })
    }

    #[test]
    fn index_array() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "arr": [10, 20, 30] }),
            json!({
                "kind": "index",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "arr" } },
                    "index": { "kind": "value", "span": null, "payload": { "value": 1 } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(20));
    }

    #[test]
    fn index_object() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "obj": { "a": 1, "b": 2 } }),
            json!({
                "kind": "index",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "obj" } },
                    "index": { "kind": "value", "span": null, "payload": { "value": "b" } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn index_object_missing_key_returns_null() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "obj": { "a": 1 } }),
            json!({
                "kind": "index",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "obj" } },
                    "index": { "kind": "value", "span": null, "payload": { "value": "z" } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(null));
    }

    #[test]
    fn index_array_out_of_bounds() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "arr": [10] }),
            json!({
                "kind": "index",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "arr" } },
                    "index": { "kind": "value", "span": null, "payload": { "value": 5 } }
                }
            }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("out of bounds"), "got: {err}");
    }

    #[test]
    fn index_string() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "s": "hello" }),
            json!({
                "kind": "index",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "s" } },
                    "index": { "kind": "value", "span": null, "payload": { "value": 1 } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("e"));
    }

    #[test]
    fn index_non_indexable() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "n": 42 }),
            json!({
                "kind": "index",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "n" } },
                    "index": { "kind": "value", "span": null, "payload": { "value": 0 } }
                }
            }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("cannot index"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalIndexPlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
