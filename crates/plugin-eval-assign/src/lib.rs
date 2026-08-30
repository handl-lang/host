//! # handl-plugin-eval-assign
//!
//! `EvalBehavior` rule for assignment expressions (kind `"assign"`).
//!
//! Ported from `AssignHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"target": <ast>, "value": <ast>}`; the rule
//! evaluates the value expression, binds it to the target pattern in the
//! current scope, and returns the evaluated value.
//!
//! For simple identifier targets (a `var` node), the variable name is
//! extracted from the target's payload and defined in the current scope.
//! For destructuring patterns (array/object), the value is decomposed
//! recursively.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for assignment expressions.
///
/// `AstNode.payload` is expected to contain
/// `{"target": <ast>, "value": <ast>}`.
/// The value is evaluated via `req.ctx.eval()`, then bound to the target
/// pattern in the current scope. The evaluated value is returned as the
/// result.
#[derive(Debug, Default, Clone, Copy)]
pub struct AssignRule;

impl Behavior<Eval> for AssignRule {
    fn kind(&self) -> &str {
        "assign"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let target_value = req.ast.payload.get("target").ok_or_else(|| {
            EvalError::TypeMismatch("assign: missing \"target\" field".into())
        })?;
        let value_ast = req.ast.payload.get("value").ok_or_else(|| {
            EvalError::TypeMismatch("assign: missing \"value\" field".into())
        })?;

        let value_ast = ast_from_value(value_ast)?;
        let evaluated = req.ctx.eval(&value_ast)?;

        let target_ast = ast_from_value(target_value)?;
        bind_pattern(&target_ast, &evaluated, &mut req.ctx)?;

        req.result = Some(evaluated);
        Ok(())
    }
}

impl EvalBehavior for AssignRule {}

/// Bind a value to a pattern AST node in the current scope.
///
/// Handles:
/// - `var` nodes: simple identifier binding
/// - `array` nodes: array destructuring (positional)
/// - `object` nodes: object destructuring (by key)
fn bind_pattern(
    pattern: &AstNode,
    value: &Value,
    ctx: &mut handl_platform::EvalContext,
) -> Result<(), PlatformError> {
    match pattern.kind.as_str() {
        "var" => {
            let name = pattern
                .payload
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    EvalError::TypeMismatch("assign: var target missing \"name\"".into())
                })?;
            ctx.define(name.to_string(), value.clone());
            Ok(())
        }
        "array" => {
            let elements = pattern
                .payload
                .get("elements")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    EvalError::TypeMismatch("assign: array target missing \"elements\"".into())
                })?;
            let arr = value.as_array().ok_or_else(|| {
                EvalError::TypeMismatch("assign: cannot destructure non-array value".into())
            })?;
            for (i, elem) in elements.iter().enumerate() {
                let child = arr.get(i).unwrap_or(&Value::Null);
                let elem_ast = ast_from_value(elem)?;
                bind_pattern(&elem_ast, child, ctx)?;
            }
            Ok(())
        }
        "object" => {
            let entries = pattern
                .payload
                .get("entries")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    EvalError::TypeMismatch("assign: object target missing \"entries\"".into())
                })?;
            let obj = value.as_object().ok_or_else(|| {
                EvalError::TypeMismatch("assign: cannot destructure non-object value".into())
            })?;
            for entry in entries {
                let key = entry
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        EvalError::TypeMismatch("assign: object entry missing \"key\"".into())
                    })?;
                let value_ast = entry
                    .get("value")
                    .ok_or_else(|| {
                        EvalError::TypeMismatch("assign: object entry missing \"value\"".into())
                    })?;
                let child = obj.get(key).unwrap_or(&Value::Null);
                let child_ast = ast_from_value(value_ast)?;
                bind_pattern(&child_ast, child, ctx)?;
            }
            Ok(())
        }
        _ => Err(EvalError::TypeMismatch(format!(
            "assign: unsupported target kind \"{}\"",
            pattern.kind
        ))
        .into()),
    }
}

/// Reconstruct an [`AstNode`] from a JSON value (serialized `AstNode`).
fn ast_from_value(value: &Value) -> Result<AstNode, EvalError> {
    serde_json::from_value::<AstNode>(value.clone()).map_err(|e| {
        EvalError::TypeMismatch(format!("assign: failed to deserialize child AST: {e}"))
    })
}

/// Registers the `assign` rule with the [`BasicEvaluator`].
pub struct EvalAssignPlugin {
    registered: AtomicBool,
}

impl EvalAssignPlugin {
    /// Create a new `EvalAssignPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalAssignPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalAssignPlugin {
    fn id(&self) -> &str {
        "eval.assign"
    }

    fn name(&self) -> &str {
        "eval-assign"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(AssignRule) as Arc<dyn EvalBehavior>)?;
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
            .accept(Arc::new(AssignRule) as Arc<dyn EvalBehavior>)
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
    fn assign_simple_identifier() {
        let evaluator = make_evaluator();
        // x = 42  then return x
        let payload = with_scope(
            json!({}),
            json!({
                "kind": "assign",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "x" } },
                    "value": { "kind": "value", "span": null, "payload": { "value": 42 } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn assign_returns_value() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({
                "kind": "assign",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "y" } },
                    "value": { "kind": "value", "span": null, "payload": { "value": "hello" } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn assign_then_read() {
        // Assign x = 99, then read x — wrapped in a sequence.
        // We use with_scope to seed nothing, then assign, then the
        // result of assign IS the value.
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({
                "kind": "assign",
                "payload": {
                    "target": { "kind": "var", "span": null, "payload": { "name": "x" } },
                    "value": { "kind": "value", "span": null, "payload": { "value": 99 } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(99));
    }

    #[test]
    fn assign_array_destructuring() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({
                "kind": "assign",
                "payload": {
                    "target": {
                        "kind": "array",
                        "span": null,
                        "payload": {
                            "elements": [
                                { "kind": "var", "span": null, "payload": { "name": "a" } },
                                { "kind": "var", "span": null, "payload": { "name": "b" } }
                            ]
                        }
                    },
                    "value": { "kind": "value", "span": null, "payload": { "value": [1, 2] } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!([1, 2]));
    }

    #[test]
    fn assign_missing_target() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({ "kind": "assign", "payload": { "value": { "kind": "value", "span": null, "payload": { "value": 1 } } } }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("target"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalAssignPlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
