//! # handl-plugin-eval-handle
//!
//! `EvalBehavior` rule for `handle` expressions (kind `"handle"`).
//!
//! Ported from `HandleHandler` in `plugin-vm-backend-rust-interpreter`.
//!
//! ## Effect-handler semantics
//!
//! A `handle` expression sets up handler clauses, then evaluates a subject
//! expression. When an operation (effect) is performed inside the subject —
//! a call to a name that has no registered function but matches a handler
//! clause — the matching handler runs instead. The handler's result replaces
//! the operation's result in the surrounding computation.
//!
//! If the handler body calls `resume(value)`, the value is used as the
//! operation's result and the computation continues. In the tree-walking
//! interpreter (which uses the Rust call stack), `resume` is an identity
//! function: it returns its argument. The parent computation — already on
//! the Rust call stack — continues with that value. This matches the
//! behaviour of the original interpreter's `eval` path for all existing
//! tests. Full continuation resumability (where `resume` returns the result
//! of the resumed computation, allowing work after `resume`) requires the
//! explicit frame-stack `eval_resumable` mode and is not needed for the
//! current test suite.
//!
//! ## AST payload
//!
//! The grammar produces `{"expr": <ast>}`. This rule also accepts an optional
//! `"handlers"` array for full effect-handler semantics:
//!
//! ```json
//! {
//!     "expr": <ast>,
//!     "handlers": [
//!         {
//!             "operation": "fetch",
//!             "params": ["url"],
//!             "resume": "k",
//!             "body": <ast>
//!         }
//!     ]
//! }
//! ```
//!
//! When `"handlers"` is absent or empty, the rule simply evaluates the inner
//! expression (backward-compatible with the step-5 implementation).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    HandlerClause, PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for `handle` expressions.
///
/// Pushes handler clauses onto `ctx.handlers`, evaluates the subject
/// expression, then pops the handler clauses. When a child call expression
/// performs an operation matching a handler clause, the call rule dispatches
/// to the handler body (see `plugin-eval-call`).
#[derive(Debug, Default, Clone, Copy)]
pub struct HandleRule;

impl Behavior<Eval> for HandleRule {
    fn kind(&self) -> &str {
        "handle"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        // The subject expression — `payload["expr"]` (required).
        let expr_value = req
            .ast
            .payload
            .get("expr")
            .ok_or_else(|| EvalError::TypeMismatch("handle: missing \"expr\" field".into()))?;
        let expr_ast = ast_from_value(expr_value)?;

        // Parse handler clauses from `payload["handlers"]` (optional).
        let clauses = parse_handler_clauses(&req.ast.payload)?;

        // Push handler clauses onto the context so that calls inside the
        // subject can find them via `ctx.find_handler`.
        req.ctx.push_handlers(clauses);

        // Evaluate the subject. If a child expression performs an operation,
        // the call rule intercepts it and runs the matching handler body.
        // The handler body's result replaces the operation's result.
        let result = req.ctx.eval(&expr_ast);

        // Always pop the handler frame, even on error, so the stack stays
        // balanced.
        req.ctx.pop_handlers();

        let result = result?;
        req.result = Some(result);
        Ok(())
    }
}

impl EvalBehavior for HandleRule {}

/// Parse handler clauses from the `handle` payload's optional `"handlers"`
/// array.
///
/// Each clause is expected to be:
/// ```json
/// { "operation": "name", "params": ["p1", ...], "resume": "k", "body": <ast> }
/// ```
///
/// `params` defaults to an empty list. `resume` defaults to `"resume"`.
/// `operation` and `body` are required.
fn parse_handler_clauses(payload: &Value) -> Result<Vec<HandlerClause>, EvalError> {
    let Some(handlers_value) = payload.get("handlers") else {
        return Ok(Vec::new());
    };
    let handlers_arr = handlers_value
        .as_array()
        .ok_or_else(|| EvalError::TypeMismatch("handle: \"handlers\" must be an array".into()))?;

    let mut clauses = Vec::with_capacity(handlers_arr.len());
    for clause_value in handlers_arr {
        let obj = clause_value
            .as_object()
            .ok_or_else(|| EvalError::TypeMismatch("handle: handler clause must be an object".into()))?;

        let operation = obj
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                EvalError::TypeMismatch("handle: handler clause missing \"operation\"".into())
            })?
            .to_string();

        let params: Vec<String> = obj
            .get("params")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let resume_name = obj
            .get("resume")
            .and_then(Value::as_str)
            .unwrap_or("resume")
            .to_string();

        let body_value = obj
            .get("body")
            .ok_or_else(|| {
                EvalError::TypeMismatch("handle: handler clause missing \"body\"".into())
            })?;
        let body = ast_from_value(body_value)?;

        clauses.push(HandlerClause {
            operation,
            params,
            resume_name,
            body,
        });
    }
    Ok(clauses)
}

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

    // --- test helper rules ---

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

    /// A test-helper `Behavior<Eval>` rule (kind `"call"`) that supports
    /// effect-handler dispatch. When the callee is a `var` node and the
    /// name is not a bound variable and not a module function, it checks
    /// `ctx.find_handler(name)`. If a handler matches, it evaluates the
    /// handler body with the call's args bound to the handler's params and
    /// the resume name bound to an identity function.
    struct CallWithHandlers;
    impl Behavior<Eval> for CallWithHandlers {
        fn kind(&self) -> &str {
            "call"
        }
        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            let callee_value = req
                .ast
                .payload
                .get("callee")
                .ok_or_else(|| EvalError::TypeMismatch("call: missing \"callee\"".into()))?;
            let args = req
                .ast
                .payload
                .get("args")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    EvalError::TypeMismatch("call: missing or non-array \"args\"".into())
                })?;

            // Evaluate arguments.
            let mut evaluated_args = Vec::with_capacity(args.len());
            for arg in args {
                let arg_ast = serde_json::from_value::<AstNode>(arg.clone())
                    .map_err(|e| PlatformError::Plugin(e.to_string()))?;
                evaluated_args.push(req.ctx.eval(&arg_ast)?);
            }

            // If the callee is a var node, try handler dispatch.
            if let Ok(callee_ast) = serde_json::from_value::<AstNode>(callee_value.clone()) {
                if callee_ast.kind == "var" {
                    if let Some(name) = callee_ast.payload.get("name").and_then(Value::as_str) {
                        // Check for a matching handler. Clone the clause
                        // to avoid holding an immutable borrow of `req.ctx`
                        // while we mutate it below.
                        if let Some(clause) = req.ctx.find_handler(name).cloned() {
                            // Bind the handler's params and resume name.
                            req.ctx.push_scope();
                            for (i, param) in clause.params.iter().enumerate() {
                                if let Some(arg) = evaluated_args.get(i) {
                                    req.ctx.define(param.clone(), arg.clone());
                                }
                            }
                            // Bind resume name to an identity function so
                            // `k(value)` returns `value`.
                            req.ctx.define(&clause.resume_name, identity_function());
                            let result = req.ctx.eval(&clause.body);
                            req.ctx.pop_scope();
                            req.result = Some(result?);
                            return Ok(());
                        }
                    }
                }
            }

            // No handler matched — try variable lookup (for bound functions).
            if let Ok(callee_ast) = serde_json::from_value::<AstNode>(callee_value.clone()) {
                if callee_ast.kind == "var" {
                    if let Some(name) = callee_ast.payload.get("name").and_then(Value::as_str) {
                        // If the variable is a function value, delegate
                        // to the `apply` primitive.
                        if let Ok(value) = req.ctx.get(name) {
                            if value.is_object()
                                && value
                                    .as_object()
                                    .map(|o| o.contains_key("params") && o.contains_key("body"))
                                    .unwrap_or(false)
                            {
                                let apply_payload = json!({ "func": value, "args": args });
                                let apply_node = AstNode {
                                    kind: "apply".to_string(),
                                    span: None,
                                    payload: apply_payload,
                                };
                                let result = req.ctx.eval(&apply_node)?;
                                req.result = Some(result);
                                return Ok(());
                            }
                            // Not a function value — return as-is.
                            req.result = Some(value);
                            return Ok(());
                        }

                        // Try builtin functions (for testing).
                        if name == "add" {
                            let sum: f64 = evaluated_args
                                .iter()
                                .filter_map(|v| v.as_number())
                                .map(|n| n.as_f64().unwrap())
                                .sum();
                            req.result = Some(json!(sum));
                            return Ok(());
                        }
                    }
                }
            }

            // Extract the callee name for a better error message.
            let callee_name = serde_json::from_value::<AstNode>(callee_value.clone())
                .ok()
                .and_then(|ast| {
                    if ast.kind == "var" {
                        ast.payload.get("name").and_then(Value::as_str).map(String::from)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "unknown".to_string());

            Err(EvalError::UnknownFunction(format!(
                "call: no handler or function for \"{callee_name}\""
            ))
            .into())
        }
    }
    impl EvalBehavior for CallWithHandlers {}

    /// A minimal `apply` primitive (kind `"apply"`) for testing resume.
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
                .ok_or_else(|| EvalError::TypeMismatch("apply: missing \"args\"".into()))?;

            let func_obj = func
                .as_object()
                .ok_or_else(|| EvalError::TypeMismatch("apply: func must be an object".into()))?;
            let params = func_obj
                .get("params")
                .and_then(Value::as_array)
                .ok_or_else(|| EvalError::TypeMismatch("apply: missing \"params\"".into()))?;
            let body = func_obj
                .get("body")
                .ok_or_else(|| EvalError::TypeMismatch("apply: missing \"body\"".into()))?;
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
                    .ok_or_else(|| EvalError::TypeMismatch("apply: bad param".into()))?;
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
            let child = req
                .ast
                .payload
                .get("child")
                .cloned()
                .unwrap_or(Value::Null);
            let child_ast = serde_json::from_value::<AstNode>(child)
                .map_err(|e| PlatformError::Plugin(e.to_string()))?;
            let result = req.ctx.eval(&child_ast)?;
            req.result = Some(result);
            Ok(())
        }
    }
    impl EvalBehavior for WithScope {}

    // --- helpers ---

    /// Create an identity function value for the resume continuation.
    /// `resume(value)` returns `value`.
    fn identity_function() -> Value {
        json!({
            "params": [["__resume_value", null]],
            "body": { "kind": "var", "span": null, "payload": { "name": "__resume_value" } },
            "captured": {}
        })
    }

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
            .accept(Arc::new(Var) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(CallWithHandlers) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(Apply) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(WithScope) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    fn var(name: &str) -> Value {
        json!({ "kind": "var", "span": null, "payload": { "name": name } })
    }

    fn val(v: impl Into<Value>) -> Value {
        json!({ "kind": "value", "span": null, "payload": { "value": v.into() } })
    }

    fn call(callee: Value, args: Vec<Value>) -> Value {
        json!({ "kind": "call", "span": null, "payload": { "callee": callee, "args": args } })
    }

    /// Build a handle expression payload for the top-level request. Note:
    /// `expr` and `handlers` are at the top level (not nested in `payload`)
    /// because `BasicEvaluator::run` uses the entire request payload as
    /// `AstNode.payload`.
    fn handle(expr: Value, handlers: Vec<Value>) -> Value {
        let mut payload = serde_json::Map::new();
        payload.insert("kind".to_string(), json!("handle"));
        payload.insert("expr".to_string(), expr);
        if !handlers.is_empty() {
            payload.insert("handlers".to_string(), Value::Array(handlers));
        }
        Value::Object(payload)
    }

    /// Build a handle expression as a nested AST node (with `payload` wrapper).
    /// Used when a handle expression appears inside another expression.
    fn handle_node(expr: Value, handlers: Vec<Value>) -> Value {
        let mut inner = serde_json::Map::new();
        inner.insert("expr".to_string(), expr);
        if !handlers.is_empty() {
            inner.insert("handlers".to_string(), Value::Array(handlers));
        }
        json!({ "kind": "handle", "span": null, "payload": Value::Object(inner) })
    }

    fn handler(operation: &str, params: Vec<&str>, resume: &str, body: Value) -> Value {
        json!({
            "operation": operation,
            "params": params,
            "resume": resume,
            "body": body
        })
    }

    // --- backward compatibility tests ---

    #[test]
    fn handle_evaluates_inner_expr_no_handlers() {
        let evaluator = make_evaluator();
        let payload = handle(val(42), vec![]);
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn handle_evaluates_string() {
        let evaluator = make_evaluator();
        let payload = handle(val("hello"), vec![]);
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

    // --- basic effect-handler tests ---

    #[test]
    fn handler_intercepts_operation() {
        // handle(fetch("data")) with handler that returns a constant
        let payload = handle(
            call(var("fetch"), vec![val("data")]),
            vec![handler(
                "fetch",
                vec!["url"],
                "k",
                val("intercepted"),
            )],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("intercepted"));
    }

    #[test]
    fn handler_body_is_pure_value() {
        // handle(42) — body is a pure value, no operations.
        // Handler is never triggered.
        let payload = handle(
            val(42),
            vec![handler("fetch", vec![], "k", val("never-called"))],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn handler_no_match_returns_unknown_function() {
        // handle(fetch("data")) with handler for "read_file" (not "fetch")
        // fetch should propagate as UnknownFunction.
        let payload = handle(
            call(var("fetch"), vec![val("data")]),
            vec![handler(
                "read_file",
                vec!["path"],
                "k",
                val("unused"),
            )],
        );
        let evaluator = make_evaluator();
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("fetch"), "got: {err}");
    }

    // --- resumable handler tests ---

    #[test]
    fn handler_resume_continuation() {
        // handle(add(fetch("data"), 10))
        // fetch signals, handler resumes k with value 42
        // result should be add(42, 10) = 52
        let payload = handle(
            call(
                var("add"),
                vec![
                    call(var("fetch"), vec![val("data")]),
                    val(10),
                ],
            ),
            vec![handler(
                "fetch",
                vec!["url"],
                "k",
                // Resume the continuation with value 42
                call(var("k"), vec![val(42)]),
            )],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(52.0));
    }

    #[test]
    fn handler_does_not_resume_continuation() {
        // Handler ignores the continuation and returns a constant.
        // add(fetch("data"), 10) → handler returns 999 → add(999, 10) = 1009
        let payload = handle(
            call(
                var("add"),
                vec![
                    call(var("fetch"), vec![val("data")]),
                    val(10),
                ],
            ),
            vec![handler(
                "fetch",
                vec!["url"],
                "k",
                // Don't call k — just return a constant
                val(999),
            )],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(1009.0));
    }

    #[test]
    fn handler_resume_with_transformed_value() {
        // Handler resumes k with a computed value (not just a constant)
        // handle(add(fetch("data"), 1))
        // handler: k(add(40, 2)) → add(42, 1) = 43
        let payload = handle(
            call(
                var("add"),
                vec![
                    call(var("fetch"), vec![val("data")]),
                    val(1),
                ],
            ),
            vec![handler(
                "fetch",
                vec!["url"],
                "k",
                call(
                    var("k"),
                    vec![call(var("add"), vec![val(40), val(2)])],
                ),
            )],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(43.0));
    }

    // --- nested handler tests ---

    #[test]
    fn nested_handlers_outer_takes_precedence() {
        // Outer handler for "fetch", inner handler for "fetch"
        // Outer wins because handler search starts from the outermost frame.
        let inner = handle_node(
            call(var("fetch"), vec![val("data")]),
            vec![handler("fetch", vec![], "k", val("inner"))],
        );
        let payload = handle(inner, vec![handler("fetch", vec![], "k", val("outer"))]);
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("outer"));
    }

    #[test]
    fn nested_handlers_outer_catches_unhandled_op() {
        // Inner handler for "read_file", outer handler for "fetch"
        // fetch is not handled by inner → propagates to outer
        let inner = handle_node(
            call(var("fetch"), vec![val("data")]),
            vec![handler("read_file", vec![], "k", val("file-handled"))],
        );
        let payload = handle(
            inner,
            vec![handler("fetch", vec![], "k", val("fetch-handled"))],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("fetch-handled"));
    }

    // --- multiple clause tests ---

    #[test]
    fn handler_multiple_clauses_last_match_wins() {
        // Two clauses for "fetch" — last one wins (handler search uses .rev())
        let payload = handle(
            call(var("fetch"), vec![val("data")]),
            vec![
                handler("fetch", vec![], "k", val("first")),
                handler("fetch", vec![], "k", val("second")),
            ],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("second"));
    }

    #[test]
    fn handler_multiple_clauses_different_ops() {
        // Two clauses for different ops — only matching one fires
        let payload = handle(
            call(var("fetch"), vec![val("data")]),
            vec![
                handler("read_file", vec![], "k", val("file-handled")),
                handler("fetch", vec![], "k", val("fetch-handled")),
            ],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("fetch-handled"));
    }

    // --- handler param binding test ---

    #[test]
    fn handler_binds_operation_args() {
        // handle(fetch("http://example.com")) with handler that returns the url
        let payload = handle(
            call(var("fetch"), vec![val("http://example.com")]),
            vec![handler(
                "fetch",
                vec!["url"],
                "k",
                var("url"),
            )],
        );
        let evaluator = make_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("http://example.com"));
    }

    // --- plugin registration test ---

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
