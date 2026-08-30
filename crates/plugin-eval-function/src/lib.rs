//! # handl-plugin-eval-function
//!
//! `EvalBehavior` rule for function literal expressions (kind `"function"`).
//!
//! Ported from `FunctionHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"params": ["x", "y", ...], "body": <ast>}`; the
//! rule creates a function value (JSON object) following the schema from
//! ADR 0040:
//!
//! ```json
//! { "params": [["x", null], ...], "body": <ast>, "captured": { ... } }
//! ```
//!
//! The function value captures all variables currently in scope. When
//! later applied (via `plugin-eval-core`'s `apply` primitive), the params
//! are bound, captured vars are merged, and the body is evaluated.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::{Map, Value};

/// Evaluation rule for function literal expressions.
///
/// `AstNode.payload` is expected to contain
/// `{"params": ["x", "y", ...], "body": <ast>}`.
/// The rule constructs a function value JSON object:
/// ```json
/// { "params": [["x", null], ...], "body": <ast>, "captured": { ... } }
/// ```
/// All variables currently in scope are captured into the `"captured"` map.
#[derive(Debug, Default, Clone, Copy)]
pub struct FunctionRule;

impl Behavior<Eval> for FunctionRule {
    fn kind(&self) -> &str {
        "function"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let params = req
            .ast
            .payload
            .get("params")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EvalError::TypeMismatch("function: missing or non-array \"params\" field".into())
            })?;

        let body = req
            .ast
            .payload
            .get("body")
            .ok_or_else(|| {
                EvalError::TypeMismatch("function: missing \"body\" field".into())
            })?;

        // Build params as [["name", null], ...] pairs.
        let params_json: Vec<Value> = params
            .iter()
            .map(|p| match p {
                Value::String(_) => Value::Array(vec![p.clone(), Value::Null]),
                Value::Array(arr) if arr.len() >= 2 => p.clone(),
                Value::Array(arr) if arr.len() == 1 => {
                    Value::Array(vec![arr[0].clone(), Value::Null])
                }
                _ => Value::Array(vec![p.clone(), Value::Null]),
            })
            .collect();

        // Capture all variables from all scopes.
        let captured: Map<String, Value> = req
            .ctx
            .collect_vars()
            .into_iter()
            .collect();

        let function_value = serde_json::json!({
            "params": params_json,
            "body": body,
            "captured": captured,
        });

        req.result = Some(function_value);
        Ok(())
    }
}

impl EvalBehavior for FunctionRule {}

/// Registers the `function` rule with the [`BasicEvaluator`].
pub struct EvalFunctionPlugin {
    registered: AtomicBool,
}

impl EvalFunctionPlugin {
    /// Create a new `EvalFunctionPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalFunctionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalFunctionPlugin {
    fn id(&self) -> &str {
        "eval.function"
    }

    fn name(&self) -> &str {
        "eval-function"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(FunctionRule) as Arc<dyn EvalBehavior>)?;
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
            let child_ast = serde_json::from_value::<handl_platform::AstNode>(child)
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            let result = req.ctx.eval(&child_ast)?;
            req.result = Some(result);
            Ok(())
        }
    }
    impl EvalBehavior for WithScope {}

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
            .accept(Arc::new(FunctionRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(WithScope) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    fn with_scope(globals: Value, child: Value) -> Value {
        json!({ "kind": "with_scope", "globals": globals, "child": child })
    }

    #[test]
    fn function_no_params() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({
                "kind": "function",
                "payload": {
                    "params": [],
                    "body": { "kind": "value", "span": null, "payload": { "value": 42 } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result["params"], json!([]));
        assert_eq!(result["body"]["kind"], "value");
        assert_eq!(result["body"]["payload"]["value"], 42);
        assert!(result["captured"].is_object());
    }

    #[test]
    fn function_with_params() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({
                "kind": "function",
                "payload": {
                    "params": ["x", "y"],
                    "body": { "kind": "var", "span": null, "payload": { "name": "x" } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result["params"], json!([["x", null], ["y", null]]));
    }

    #[test]
    fn function_captures_scope() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "z": 99 }),
            json!({
                "kind": "function",
                "payload": {
                    "params": ["x"],
                    "body": { "kind": "var", "span": null, "payload": { "name": "z" } }
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result["captured"]["z"], json!(99));
    }

    #[test]
    fn function_missing_params() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({ "kind": "function", "payload": { "body": { "kind": "value", "span": null, "payload": { "value": 1 } } } }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("params"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalFunctionPlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
