//! # handl-plugin-eval-call
//!
//! `EvalBehavior` rule for function call expressions (kind `"call"`).
//!
//! Ported from `CallHandler` in `plugin-vm-backend-rust-interpreter`.
//! The AST payload is `{"callee": <ast>, "args": [<ast>, ...]}`; the rule
//! evaluates the callee to obtain a function value, then delegates to the
//! `apply` primitive (kind `"apply"` in `plugin-eval-core`) by constructing
//! an apply AST node and dispatching it through `ctx.eval()`.
//!
//! If the callee evaluates to a string, it is treated as a variable name
//! and looked up in the scope stack to find the function value. If it
//! evaluates to a function value (JSON object with `params`/`body`), it is
//! used directly.
//!
//! ## Effect-handler dispatch
//!
//! When the callee is a simple `var` node and the name is not a bound
//! variable and not a module-qualified function, the rule checks
//! `ctx.find_handler(name)` for a matching effect handler. If a handler
//! matches, the handler body is evaluated with the call's args bound to the
//! handler's params and the resume name bound to an identity function (so
//! `resume(value)` returns `value`). The handler body's result replaces the
//! operation's result in the surrounding computation.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for function call expressions.
///
/// `AstNode.payload` is expected to contain
/// `{"callee": <ast>, "args": [<ast>, ...]}`.
/// The callee is evaluated first. If the result is a string, it is looked
/// up as a variable name to find the function value. If the result is a
/// function value (JSON object with `params`/`body`), it is used directly.
/// The arguments are passed as unevaluated ASTs to the `apply` primitive,
/// which evaluates them and binds them to the function's parameters.
#[derive(Debug, Default, Clone, Copy)]
pub struct CallRule;

impl Behavior<Eval> for CallRule {
    fn kind(&self) -> &str {
        "call"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let callee_value = req.ast.payload.get("callee").ok_or_else(|| {
            EvalError::TypeMismatch("call: missing \"callee\" field".into())
        })?;
        let args = req
            .ast
            .payload
            .get("args")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EvalError::TypeMismatch("call: missing or non-array \"args\" field".into())
            })?;

        // Try module-qualified call first: if the callee is an `index`
        // expression with a `var` target and a string literal index
        // (e.g. `hello.greet`), attempt `ctx.call_function("hello.greet", args)`.
        if let Some(qualified_name) = try_qualified_name(callee_value) {
            // Evaluate the arguments first.
            let mut evaluated_args = Vec::with_capacity(args.len());
            for arg in args {
                let arg_ast = ast_from_value(arg)?;
                evaluated_args.push(req.ctx.eval(&arg_ast)?);
            }
            let result = req.ctx.call_function(&qualified_name, &evaluated_args)?;
            req.result = Some(result);
            return Ok(());
        }

        // Effect-handler dispatch: if the callee is a simple `var` node,
        // check for a matching handler before trying variable lookup.
        // This intercepts operation (effect) calls inside `handle` expressions.
        if let Some(op_name) = try_var_name(callee_value) {
            if let Some(clause) = req.ctx.find_handler(&op_name).cloned() {
                // Evaluate the arguments.
                let mut evaluated_args = Vec::with_capacity(args.len());
                for arg in args {
                    let arg_ast = ast_from_value(arg)?;
                    evaluated_args.push(req.ctx.eval(&arg_ast)?);
                }
                // Bind handler params and resume name in a fresh scope.
                req.ctx.push_scope();
                for (i, param) in clause.params.iter().enumerate() {
                    if let Some(arg) = evaluated_args.get(i) {
                        req.ctx.define(param.clone(), arg.clone());
                    }
                }
                // Bind the resume name to an identity function so
                // `resume(value)` returns `value`. In the tree-walking
                // interpreter the parent computation is already on the
                // Rust call stack, so the handler body's result replaces
                // the operation's result and the computation continues.
                req.ctx.define(&clause.resume_name, identity_function());
                let result = req.ctx.eval(&clause.body);
                req.ctx.pop_scope();
                req.result = Some(result?);
                return Ok(());
            }
        }

        // Evaluate the callee to get a function value.
        let callee_ast = ast_from_value(callee_value)?;
        let callee_result = req.ctx.eval(&callee_ast)?;

        // If the callee evaluated to a string, look it up as a variable.
        let func_value = match &callee_result {
            Value::String(name) => {
                req.ctx
                    .get(name)
                    .map_err(|_| {
                        EvalError::UnknownFunction(format!(
                            "call: callee \"{name}\" is not a bound function"
                        ))
                    })?
            }
            Value::Object(obj) if obj.contains_key("params") && obj.contains_key("body") => {
                callee_result
            }
            _ => {
                return Err(EvalError::TypeMismatch(format!(
                    "call: callee must be a function name or function value, got {}",
                    json_kind(&callee_result)
                ))
                .into());
            }
        };

        // Delegate to the `apply` primitive by constructing an apply AST
        // node and dispatching it through ctx.eval().
        let apply_payload = serde_json::json!({
            "func": func_value,
            "args": args,
        });
        let apply_node = AstNode {
            kind: "apply".to_string(),
            span: None,
            payload: apply_payload,
        };
        let result = req.ctx.eval(&apply_node)?;
        req.result = Some(result);
        Ok(())
    }
}

impl EvalBehavior for CallRule {}

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
        EvalError::TypeMismatch(format!("call: failed to deserialize child AST: {e}"))
    })
}

/// Check if `callee_value` represents a module-qualified name like
/// `hello.greet` (parsed as an `index` AST with a `var` target and a
/// string literal index). Returns the qualified name (`"hello.greet"`)
/// if so, or `None` otherwise.
fn try_qualified_name(callee_value: &Value) -> Option<String> {
    let callee_ast = serde_json::from_value::<AstNode>(callee_value.clone()).ok()?;

    if callee_ast.kind != "index" {
        return None;
    }

    // The target must be a `var` node.
    let target_value = callee_ast.payload.get("target")?;
    let target_ast = serde_json::from_value::<AstNode>(target_value.clone()).ok()?;
    if target_ast.kind != "var" {
        return None;
    }
    let module_name = target_ast.payload.get("name")?.as_str()?.to_string();

    // The index must be a `value` node with a string payload.
    let index_value = callee_ast.payload.get("index")?;
    let index_ast = serde_json::from_value::<AstNode>(index_value.clone()).ok()?;
    if index_ast.kind != "value" {
        return None;
    }
    let func_name = index_ast.payload.get("value")?.as_str()?.to_string();

    Some(format!("{module_name}.{func_name}"))
}

/// If `callee_value` is a `var` AST node, return its name. Otherwise `None`.
///
/// Used to check for effect-handler dispatch before evaluating the callee.
fn try_var_name(callee_value: &Value) -> Option<String> {
    let callee_ast = serde_json::from_value::<AstNode>(callee_value.clone()).ok()?;
    if callee_ast.kind != "var" {
        return None;
    }
    callee_ast
        .payload
        .get("name")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

/// Create an identity function value for the effect-handler resume
/// continuation.
///
/// `resume(value)` returns `value`. In the tree-walking interpreter the
/// parent computation is already on the Rust call stack, so the handler
/// body's result replaces the operation's result and the computation
/// continues with that value.
fn identity_function() -> Value {
    serde_json::json!({
        "params": [["__resume_value", null]],
        "body": { "kind": "var", "span": null, "payload": { "name": "__resume_value" } },
        "captured": {}
    })
}

/// Registers the `call` rule with the [`BasicEvaluator`].
pub struct EvalCallPlugin {
    registered: AtomicBool,
}

impl EvalCallPlugin {
    /// Create a new `EvalCallPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalCallPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalCallPlugin {
    fn id(&self) -> &str {
        "eval.call"
    }

    fn name(&self) -> &str {
        "eval-call"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(CallRule) as Arc<dyn EvalBehavior>)?;
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

    // --- test helpers ---

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

    /// A minimal `apply` primitive (kind `"apply"`) for testing.
    /// Mirrors `plugin-eval-core`'s `Apply` but simplified.
    struct Apply;
    impl Behavior<Eval> for Apply {
        fn kind(&self) -> &str {
            "apply"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            let func = req
                .ast
                .payload
                .get("func")
                .ok_or_else(|| EvalError::TypeMismatch("apply: missing \"func\"".into()))?
                .clone();
            let args = req
                .ast
                .payload
                .get("args")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    EvalError::TypeMismatch("apply: missing \"args\"".into())
                })?;

            let func_obj = func.as_object().ok_or_else(|| {
                EvalError::TypeMismatch("apply: func must be an object".into())
            })?;
            let params = func_obj
                .get("params")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    EvalError::TypeMismatch("apply: missing \"params\"".into())
                })?;
            let body = func_obj.get("body").ok_or_else(|| {
                EvalError::TypeMismatch("apply: missing \"body\"".into())
            })?;
            let captured = func_obj
                .get("captured")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();

            // Evaluate args.
            let mut evaluated_args = Vec::new();
            for arg in args {
                let arg_ast = serde_json::from_value::<AstNode>(arg.clone())
                    .map_err(|e| PlatformError::Plugin(e.to_string()))?;
                evaluated_args.push(req.ctx.eval(&arg_ast)?);
            }

            // Push scope, bind captured + params.
            req.ctx.push_scope();
            for (name, value) in &captured {
                req.ctx.define(name.clone(), value.clone());
            }
            for (i, param) in params.iter().enumerate() {
                let pname = param
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        EvalError::TypeMismatch("apply: bad param".into())
                    })?;
                if let Some(arg_value) = evaluated_args.get(i) {
                    req.ctx.define(pname.to_string(), arg_value.clone());
                }
            }

            // Evaluate body.
            let body_ast = serde_json::from_value::<AstNode>(body.clone())
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            let result = req.ctx.eval(&body_ast);
            req.ctx.pop_scope();
            req.result = Some(result?);
            Ok(())
        }
    }
    impl EvalBehavior for Apply {}

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
            .accept(Arc::new(CallRule) as Arc<dyn EvalBehavior>)
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
            .accept(Arc::new(Apply) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    fn with_scope(globals: Value, child: Value) -> Value {
        json!({ "kind": "with_scope", "globals": globals, "child": child })
    }

    #[test]
    fn call_function_by_name() {
        // Define f = (x) => x, then call f(5).
        let func = json!({
            "params": [["x", null]],
            "body": { "kind": "var", "span": null, "payload": { "name": "x" } },
            "captured": {}
        });
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "f": func }),
            json!({
                "kind": "call",
                "payload": {
                    "callee": { "kind": "var", "span": null, "payload": { "name": "f" } },
                    "args": [
                        { "kind": "value", "span": null, "payload": { "value": 5 } }
                    ]
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn call_function_with_captured_var() {
        // f = () => z, where z is captured.
        let func = json!({
            "params": [],
            "body": { "kind": "var", "span": null, "payload": { "name": "z" } },
            "captured": { "z": 42 }
        });
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({ "f": func }),
            json!({
                "kind": "call",
                "payload": {
                    "callee": { "kind": "var", "span": null, "payload": { "name": "f" } },
                    "args": []
                }
            }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn call_unknown_function() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({
                "kind": "call",
                "payload": {
                    "callee": { "kind": "var", "span": null, "payload": { "name": "g" } },
                    "args": []
                }
            }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("g"), "got: {err}");
    }

    #[test]
    fn call_missing_callee() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            json!({ "kind": "call", "payload": { "args": [] } }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("callee"), "got: {err}");
    }

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalCallPlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
