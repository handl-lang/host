//! # handl-plugin-eval-core
//!
//! Eval kernel minimum primitives as `Behavior<Eval>` rules.
//!
//! This crate provides the three built-in evaluation primitives that every
//! Handl expression evaluation depends on:
//!
//! - **`var`** — variable reference lookup. Looks up a variable name in the
//!   scope stack and returns its value.
//! - **`resolve`** — path resolution. Resolves a dotted path (e.g.
//!   `foo.bar.baz`) by traversing JSON objects.
//! - **`apply`** — function application. Takes a function value and
//!   arguments, binds params, evaluates the body.
//!
//! Function values are represented as JSON objects with an implicit schema
//! (see ADR 0040 — "Function values as JSON"):
//!
//! ```json
//! { "params": [["x", null], ["y", null]], "body": <ast payload>, "captured": { "z": 42 } }
//! ```
//!
//! `apply` reads this schema, binds params, merges captured vars, and calls
//! `ctx.eval(&body)`. No type safety — that's the JSON-boundary tradeoff the
//! platform already commits to (design principle #6, #8).
//!
//! Each rule is a `Behavior<Eval>` impl, and the crate exposes a single
//! [`EvalCorePlugin`] that contributes all three rules to the
//! [`BasicEvaluator`].

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

// ---------------------------------------------------------------------------
// var
// ---------------------------------------------------------------------------

/// Evaluation rule that looks up a variable reference in the scope stack.
///
/// `AstNode.payload` is expected to contain `{"name": "x"}`. The rule looks up
/// `name` in `req.ctx` and sets `req.result` to the bound value, or returns
/// [`EvalError::UnknownVariable`] if the name is not bound in any scope.
#[derive(Debug, Default, Clone, Copy)]
pub struct Var;

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
                EvalError::TypeMismatch("var: missing or non-string \"name\" field".into())
            })?;

        let value = req.ctx.get(name).map_err(PlatformError::from)?;
        req.result = Some(value);
        Ok(())
    }
}

impl EvalBehavior for Var {}

// ---------------------------------------------------------------------------
// resolve
// ---------------------------------------------------------------------------

/// Evaluation rule that resolves a dotted path against the scope stack.
///
/// `AstNode.payload` is expected to contain either:
/// - `{"path": ["foo", "bar", "baz"]}` — an array of string segments, or
/// - `{"path": "foo.bar.baz"}` — a dotted string.
///
/// The first segment is looked up in the scope stack; subsequent segments
/// traverse the resulting JSON value (objects by key, arrays by index).
#[derive(Debug, Default, Clone, Copy)]
pub struct Resolve;

impl Behavior<Eval> for Resolve {
    fn kind(&self) -> &str {
        "resolve"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let path_value = req
            .ast
            .payload
            .get("path")
            .ok_or_else(|| {
                EvalError::TypeMismatch("resolve: missing \"path\" field".into())
            })?;

        let segments: Vec<String> = match path_value {
            Value::Array(arr) => arr
                .iter()
                .map(|v| {
                    v.as_str().map(|s| s.to_string()).ok_or_else(|| {
                        EvalError::TypeMismatch(
                            "resolve: path segments must be strings".into(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            Value::String(s) => s.split('.').map(|seg| seg.to_string()).collect(),
            _ => {
                return Err(EvalError::TypeMismatch(
                    "resolve: \"path\" must be a string or array of strings".into(),
                )
                .into());
            }
        };

        if segments.is_empty() {
            return Err(EvalError::TypeMismatch("resolve: empty path".into()).into());
        }

        // Look up the first segment in the scope stack.
        let mut current = req.ctx.get(&segments[0]).map_err(PlatformError::from)?;

        // Traverse subsequent segments through the JSON value.
        for segment in &segments[1..] {
            current = traverse(&current, segment)?;
        }

        req.result = Some(current);
        Ok(())
    }
}

impl EvalBehavior for Resolve {}

/// Traverse one segment into a JSON value (object key or array index).
fn traverse(value: &Value, segment: &str) -> Result<Value, EvalError> {
    match value {
        Value::Object(obj) => obj
            .get(segment)
            .cloned()
            .ok_or_else(|| EvalError::UnknownVariable(segment.to_string())),
        Value::Array(arr) => {
            let idx = segment
                .parse::<usize>()
                .map_err(|_| EvalError::TypeMismatch(format!(
                    "resolve: cannot index array with non-numeric segment \"{segment}\""
                )))?;
            arr.get(idx)
                .cloned()
                .ok_or_else(|| EvalError::UnknownVariable(segment.to_string()))
        }
        _ => Err(EvalError::TypeMismatch(format!(
            "resolve: cannot traverse into non-object/non-array value at \"{segment}\""
        ))),
    }
}

// ---------------------------------------------------------------------------
// apply
// ---------------------------------------------------------------------------

/// Evaluation rule that applies a function value to arguments.
///
/// `AstNode.payload` is expected to contain:
/// ```json
/// { "func": <function value>, "args": [<ast>, <ast>, ...] }
/// ```
///
/// The function value is a JSON object:
/// ```json
/// { "params": [["x", null], ...], "body": <ast payload>, "captured": {...} }
/// ```
///
/// Each argument AST is evaluated via `req.ctx.eval(&arg_ast)`. A new scope is
/// pushed, params are bound to the evaluated args, captured vars are merged,
/// the body is evaluated via `req.ctx.eval(&body_ast)`, and the scope is
/// popped. The body's result becomes `req.result`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Apply;

impl Behavior<Eval> for Apply {
    fn kind(&self) -> &str {
        "apply"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let func = req
            .ast
            .payload
            .get("func")
            .ok_or_else(|| {
                EvalError::TypeMismatch("apply: missing \"func\" field".into())
            })?
            .clone();

        let args = req
            .ast
            .payload
            .get("args")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EvalError::TypeMismatch("apply: missing or non-array \"args\" field".into())
            })?;

        // Decode the function value schema.
        let func_obj = func.as_object().ok_or_else(|| {
            EvalError::TypeMismatch("apply: \"func\" must be a JSON object".into())
        })?;

        let params = func_obj
            .get("params")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EvalError::TypeMismatch("apply: function value missing \"params\" array".into())
            })?;

        let body = func_obj
            .get("body")
            .ok_or_else(|| {
                EvalError::TypeMismatch("apply: function value missing \"body\" field".into())
            })?
            .clone();

        let captured = func_obj
            .get("captured")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        // Evaluate each argument AST via the recursion handle.
        let mut evaluated_args: Vec<Value> = Vec::with_capacity(args.len());
        for arg in args {
            let arg_ast = ast_from_value(arg)?;
            let value = req.ctx.eval(&arg_ast)?;
            evaluated_args.push(value);
        }

        // Push a new scope and bind captured vars + params.
        req.ctx.push_scope();

        for (name, value) in &captured {
            req.ctx.define(name.clone(), value.clone());
        }

        for (i, param) in params.iter().enumerate() {
            let param_arr = param.as_array().ok_or_else(|| {
                EvalError::TypeMismatch("apply: each param must be a [name, default] pair".into())
            })?;
            let pname = param_arr
                .first()
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    EvalError::TypeMismatch("apply: param name must be a string".into())
                })?;

            if let Some(arg_value) = evaluated_args.get(i) {
                req.ctx.define(pname, arg_value.clone());
            } else if let Some(default) = param_arr.get(1) {
                if !default.is_null() {
                    req.ctx.define(pname, default.clone());
                }
            }
        }

        // Evaluate the body.
        let body_ast = ast_from_value(&body)?;
        let result = req.ctx.eval(&body_ast);

        // Always pop the scope, even on error.
        req.ctx.pop_scope();

        req.result = Some(result?);
        Ok(())
    }
}

impl EvalBehavior for Apply {}

/// Reconstruct an [`AstNode`] from a JSON payload.
///
/// The payload is expected to be an object with `kind` and `payload` fields:
/// ```json
/// { "kind": "var", "payload": { "name": "x" } }
/// ```
///
/// If `kind` is absent, the value itself is treated as the payload with an
/// empty kind (useful for inline literal payloads).
fn ast_from_value(value: &Value) -> Result<AstNode, EvalError> {
    if let Some(obj) = value.as_object() {
        if let Some(kind) = obj.get("kind").and_then(Value::as_str) {
            let payload = obj.get("payload").cloned().unwrap_or(Value::Null);
            return Ok(AstNode {
                kind: kind.to_string(),
                span: None,
                payload,
            });
        }
    }
    // Treat the value itself as a literal payload with an empty kind.
    Ok(AstNode {
        kind: String::new(),
        span: None,
        payload: value.clone(),
    })
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers the `var`, `resolve`, and `apply` rules with the
/// [`BasicEvaluator`].
///
/// Each rule is contributed exactly once. The plugin uses an atomic counter
/// to guard against duplicate registration across multiple `contribute` calls.
pub struct EvalCorePlugin {
    contributed: AtomicUsize,
}

impl EvalCorePlugin {
    /// Create a new `EvalCorePlugin`.
    pub fn new() -> Self {
        Self {
            contributed: AtomicUsize::new(0),
        }
    }
}

impl Default for EvalCorePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalCorePlugin {
    fn id(&self) -> &str {
        "eval.core"
    }

    fn name(&self) -> &str {
        "eval-core"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.contributed.fetch_add(1, Ordering::SeqCst) == 0 {
            ctx.contribute::<BasicEvaluator>(Arc::new(Var) as Arc<dyn EvalBehavior>)?;
            ctx.contribute::<BasicEvaluator>(Arc::new(Resolve) as Arc<dyn EvalBehavior>)?;
            ctx.contribute::<BasicEvaluator>(Arc::new(Apply) as Arc<dyn EvalBehavior>)?;
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        BasicEvaluator, CapabilityId, CapabilityRequest, DefaultHost, KitRegistry, PluginContext,
        PluginRegistry, RuleAcceptor,
    };
    use serde_json::json;

    /// A test-helper `Behavior<Eval>` rule (`kind: "with_scope"`) that seeds
    /// the context with globals read from `payload.globals`, then evaluates
    /// the child AST described by `payload.child` and forwards its result.
    ///
    /// This lets the `var`/`resolve` tests run through the public
    /// `BasicEvaluator::run` entry point (which always starts from a fresh
    /// `EvalContext`) by wrapping the node under test in a `with_scope` node.
    struct WithScope;

    impl Behavior<Eval> for WithScope {
        fn kind(&self) -> &str {
            "with_scope"
        }

        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            // Seed globals into the (global) scope.
            if let Some(globals) = req.ast.payload.get("globals").and_then(Value::as_object) {
                for (name, value) in globals {
                    req.ctx.define(name.clone(), value.clone());
                }
            }
            // Evaluate the child node.
            let child = req.ast.payload.get("child").cloned().unwrap_or(Value::Null);
            let child_ast = ast_from_value(&child)?;
            let result = req.ctx.eval(&child_ast)?;
            req.result = Some(result);
            Ok(())
        }
    }
    impl EvalBehavior for WithScope {}

    /// A test-helper `Behavior<Eval>` rule (`kind: "json"`) that parses a JSON
    /// literal from `payload.expr` and returns it. Used to produce argument
    /// values in `apply` tests.
    struct JsonLit;

    impl Behavior<Eval> for JsonLit {
        fn kind(&self) -> &str {
            "json"
        }

        fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
            let expr = req
                .ast
                .payload
                .get("expr")
                .and_then(Value::as_str)
                .ok_or_else(|| PlatformError::InvalidMetadata("missing expr".into()))?;
            let v: Value = serde_json::from_str(expr)
                .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;
            req.result = Some(v);
            Ok(())
        }
    }
    impl EvalBehavior for JsonLit {}

    /// Build a `BasicEvaluator` with the three core rules plus the test
    /// helpers (`with_scope`, `json`) registered.
    fn core_evaluator() -> BasicEvaluator {
        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(Var) as Arc<dyn EvalBehavior>).unwrap();
        evaluator
            .accept(Arc::new(Resolve) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(Apply) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(WithScope) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(JsonLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
    }

    /// Run an eval request through `evaluator.run`, returning the result
    /// value or error.
    fn run_eval(
        evaluator: &BasicEvaluator,
        payload: Value,
    ) -> Result<Value, PlatformError> {
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

    /// Wrap a child node in a `with_scope` payload that seeds the given
    /// globals before evaluating the child.
    fn with_scope(globals: Value, child: Value) -> Value {
        json!({ "kind": "with_scope", "globals": globals, "child": child })
    }

    // --- var ---------------------------------------------------------------

    #[test]
    fn var_lookup_found() {
        let evaluator = core_evaluator();
        let payload = with_scope(
            json!({ "x": 42 }),
            json!({ "kind": "var", "payload": { "name": "x" } }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn var_lookup_not_found() {
        let evaluator = core_evaluator();
        let payload = with_scope(
            json!({}),
            json!({ "kind": "var", "payload": { "name": "missing" } }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown variable: missing"),
            "expected unknown variable error, got: {msg}"
        );
    }

    #[test]
    fn var_missing_name_field() {
        let evaluator = core_evaluator();
        let payload = with_scope(json!({}), json!({ "kind": "var", "payload": {} }));
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("missing"), "got: {err}");
    }

    // --- resolve -----------------------------------------------------------

    #[test]
    fn resolve_simple_path() {
        let evaluator = core_evaluator();
        let payload = with_scope(
            json!({ "foo": 7 }),
            json!({ "kind": "resolve", "payload": { "path": ["foo"] } }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(7));
    }

    #[test]
    fn resolve_nested_path_array() {
        let evaluator = core_evaluator();
        let payload = with_scope(
            json!({ "foo": { "bar": { "baz": 99 } } }),
            json!({ "kind": "resolve", "payload": { "path": ["foo", "bar", "baz"] } }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(99));
    }

    #[test]
    fn resolve_nested_path_string() {
        let evaluator = core_evaluator();
        let payload = with_scope(
            json!({ "foo": { "bar": { "baz": "hello" } } }),
            json!({ "kind": "resolve", "payload": { "path": "foo.bar.baz" } }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn resolve_array_index() {
        let evaluator = core_evaluator();
        let payload = with_scope(
            json!({ "arr": [10, 20, 30] }),
            json!({ "kind": "resolve", "payload": { "path": ["arr", "1"] } }),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(20));
    }

    #[test]
    fn resolve_missing_segment() {
        let evaluator = core_evaluator();
        let payload = with_scope(
            json!({ "foo": { "bar": 1 } }),
            json!({ "kind": "resolve", "payload": { "path": ["foo", "nope"] } }),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("nope"), "got: {err}");
    }

    // --- apply -------------------------------------------------------------

    #[test]
    fn apply_simple_function() {
        // Function: (x) => x  (body is a var reference to "x")
        let func = json!({
            "params": [["x", null]],
            "body": { "kind": "var", "payload": { "name": "x" } },
            "captured": {}
        });
        // Argument: the literal 5 (a json literal node).
        let arg = json!({ "kind": "json", "payload": { "expr": "5" } });

        let payload = json!({
            "kind": "apply",
            "func": func,
            "args": [arg],
        });

        let evaluator = core_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn apply_with_captured_vars() {
        // Function: () => z  where z is captured.
        let func = json!({
            "params": [],
            "body": { "kind": "var", "payload": { "name": "z" } },
            "captured": { "z": 42 }
        });

        let payload = json!({ "kind": "apply", "func": func, "args": [] });

        let evaluator = core_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn apply_binds_param_over_captured() {
        // Function: (x) => x  where captured also has x=999. Param wins.
        let func = json!({
            "params": [["x", null]],
            "body": { "kind": "var", "payload": { "name": "x" } },
            "captured": { "x": 999 }
        });
        let arg = json!({ "kind": "json", "payload": { "expr": "5" } });

        let payload = json!({ "kind": "apply", "func": func, "args": [arg] });

        let evaluator = core_evaluator();
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn apply_missing_func_field() {
        let payload = json!({ "kind": "apply", "args": [] });
        let evaluator = core_evaluator();
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("func"), "got: {err}");
    }

    // --- plugin registration ----------------------------------------------

    #[test]
    fn plugin_contributes_three_rules_once() {
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalCorePlugin::new());

        // Drive the contribution phase to a fixed point.
        host.load().expect("host load should succeed");

        // The BasicEvaluator should now have exactly 3 rules.
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 3);
    }

    #[test]
    fn plugin_contribute_is_idempotent() {
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalCorePlugin::new());
        host.load().unwrap();

        // Re-running load should not duplicate rules (the plugin guards with
        // an atomic counter).
        host.load().unwrap();
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .unwrap();
        assert_eq!(evaluator.rules.len(), 3);
    }
}
