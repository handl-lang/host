//! # handl-plugin-eval-match
//!
//! `EvalBehavior` rule for `match` expressions (kind `"match"`).
//!
//! The AST payload is `{"subject": <ast>, "arms": [{"pattern": <ast>,
//! "guard": <ast|null>, "body": <ast>}, ...]}` (produced by
//! `plugin-grammar-match`). The rule evaluates the subject, then tries each
//! arm in order: the arm's pattern is matched against the subject value,
//! producing a set of bindings; if a guard is present it is evaluated in a
//! child scope holding those bindings and must be truthy; the arm's body is
//! then evaluated in the same child scope and its result returned.
//! Evaluation short-circuits on the first matching arm.
//!
//! Patterns are interpreted as expression ASTs:
//!
//! - `value` node (`{"value": <json>}`) — **literal pattern**: match by
//!   equality.
//! - `var` node (`{"name": "x"}`) — **binder**: binds the whole subject
//!   value to `x`. The name `_` is a wildcard (matches anything, binds
//!   nothing).
//! - `object` node (`{"entries": [{"key": k, "value": <sub-pattern>}, ...]}`)
//!   — **structural pattern**: the subject must be an object containing each
//!   key, and each sub-pattern must match the corresponding field. A
//!   `"kind"` key with a literal value acts as a discriminator, which is how
//!   AST traversal is expressed (e.g. matching against a serialized
//!   `AstNode` value `{"kind": "call", "payload": {...}}`).
//! - `array` node (`{"elements": [<sub-pattern>, ...]}`) — **positional
//!   pattern**: the subject must be an array of matching length, and each
//!   sub-pattern must match the element at the same index.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use handl_platform::{
    AstNode, BasicEvaluator, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Eval, EvalBehavior, EvalError, EvalRequest,
    PlatformError, Plugin, PluginContext,
};
use serde_json::Value;

/// Evaluation rule for `match` expressions.
///
/// See the crate docs for the pattern language.
#[derive(Debug, Default, Clone, Copy)]
pub struct MatchRule;

impl Behavior<Eval> for MatchRule {
    fn kind(&self) -> &str {
        "match"
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        let subject_ast = req
            .ast
            .payload
            .get("subject")
            .ok_or_else(|| {
                EvalError::TypeMismatch("match: missing \"subject\" field".into())
            })?;
        let arms = req
            .ast
            .payload
            .get("arms")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EvalError::TypeMismatch("match: missing or non-array \"arms\" field".into())
            })?;

        let subject_ast = ast_from_value(subject_ast)?;
        let subject = req.ctx.eval(&subject_ast)?;

        for arm in arms {
            let pattern_value = arm.get("pattern").ok_or_else(|| {
                EvalError::TypeMismatch("match: arm missing \"pattern\" field".into())
            })?;
            let pattern_ast = ast_from_value(pattern_value)?;

            // Try to match the pattern against the subject value.
            let bindings = match match_pattern(&pattern_ast, &subject) {
                Some(b) => b,
                None => continue,
            };

            // If a guard is present (non-null), evaluate it in a child scope
            // holding the bindings and require a truthy result.
            let guard_value = arm.get("guard");
            if let Some(guard) = guard_value {
                if !guard.is_null() {
                    let guard_ast = ast_from_value(guard)?;
                    req.ctx.push_scope();
                    for (name, value) in &bindings {
                        req.ctx.define(name.clone(), value.clone());
                    }
                    let guard_result = req.ctx.eval(&guard_ast);
                    req.ctx.pop_scope();
                    let guard_result = guard_result?;
                    if !is_truthy(&guard_result) {
                        continue;
                    }
                }
            }

            // Matched: evaluate the body in a child scope with the bindings.
            let body_value = arm.get("body").ok_or_else(|| {
                EvalError::TypeMismatch("match: arm missing \"body\" field".into())
            })?;
            let body_ast = ast_from_value(body_value)?;

            req.ctx.push_scope();
            for (name, value) in &bindings {
                req.ctx.define(name.clone(), value.clone());
            }
            let result = req.ctx.eval(&body_ast);
            req.ctx.pop_scope();

            req.result = Some(result?);
            return Ok(());
        }

        Err(EvalError::TypeMismatch(format!(
            "match: no arm matched subject {}",
            summarize(&subject)
        ))
        .into())
    }
}

impl EvalBehavior for MatchRule {}

/// A JSON value is truthy if it is not `null`, `false`, `0`, `0.0`, or the
/// empty string. (Mirrors the convention used elsewhere in the evaluator.)
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Short, human-readable summary of a value for error messages.
fn summarize(v: &Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_else(|_| "<unserializable>".into());
    if s.len() > 80 {
        format!("{}…", &s[..80])
    } else {
        s
    }
}

/// Match a pattern AST against a value, returning the bindings it produces
/// or `None` if it does not match.
///
/// Bindings are a flat list of `(name, value)` pairs; later bindings shadow
/// earlier ones with the same name (callers define them in order).
fn match_pattern(pattern: &AstNode, value: &Value) -> Option<Vec<(String, Value)>> {
    match pattern.kind.as_str() {
        // Literal pattern: match by equality.
        "value" => {
            let lit = pattern.payload.get("value").unwrap_or(&Value::Null);
            if lit == value {
                Some(Vec::new())
            } else {
                None
            }
        }

        // Binder (or wildcard).
        "var" => {
            let name = pattern
                .payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("_");
            if name == "_" {
                Some(Vec::new())
            } else {
                Some(vec![(name.to_string(), value.clone())])
            }
        }

        // Structural object pattern.
        "object" => {
            let entries = pattern.payload.get("entries").and_then(Value::as_array)?;
            let obj = value.as_object()?;
            let mut bindings = Vec::new();
            for entry in entries {
                let key = entry.get("key").and_then(Value::as_str)?;
                let sub_pattern_value = entry.get("value")?;
                let sub_pattern = ast_from_value(sub_pattern_value).ok()?;
                let field = obj.get(key)?;
                match match_pattern(&sub_pattern, field) {
                    Some(sub_bindings) => bindings.extend(sub_bindings),
                    None => return None,
                }
            }
            Some(bindings)
        }

        // Positional array pattern.
        "array" => {
            let elements = pattern.payload.get("elements").and_then(Value::as_array)?;
            let arr = value.as_array()?;
            if arr.len() != elements.len() {
                return None;
            }
            let mut bindings = Vec::new();
            for (elem_pattern, elem_value) in elements.iter().zip(arr.iter()) {
                let sub_pattern = ast_from_value(elem_pattern).ok()?;
                match match_pattern(&sub_pattern, elem_value) {
                    Some(sub_bindings) => bindings.extend(sub_bindings),
                    None => return None,
                }
            }
            Some(bindings)
        }

        // Unknown pattern kind: treat the payload itself as a literal to
        // compare against (defensive; real patterns are always one of the
        // above).
        _ => {
            if pattern.payload == *value {
                Some(Vec::new())
            } else {
                None
            }
        }
    }
}

/// Reconstruct an [`AstNode`] from a JSON value (serialized `AstNode`).
fn ast_from_value(value: &Value) -> Result<AstNode, EvalError> {
    serde_json::from_value::<AstNode>(value.clone()).map_err(|e| {
        EvalError::TypeMismatch(format!("match: failed to deserialize child AST: {e}"))
    })
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers the `match` rule with the [`BasicEvaluator`].
pub struct EvalMatchPlugin {
    registered: AtomicBool,
}

impl EvalMatchPlugin {
    /// Create a new `EvalMatchPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for EvalMatchPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EvalMatchPlugin {
    fn id(&self) -> &str {
        "eval.match"
    }

    fn name(&self) -> &str {
        "eval-match"
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
            ctx.contribute::<BasicEvaluator>(Arc::new(MatchRule) as Arc<dyn EvalBehavior>)?;
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
        BasicEvaluator, CapabilityId, CapabilityRequest, KitRegistry, PluginContext,
        PluginRegistry, RuleAcceptor,
    };
    use serde_json::json;

    // --- test helpers (mirrors the pattern used by other eval plugins) ---

    /// Seeds globals then evaluates a child node.
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

    /// Literal value rule (kind `"value"`).
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

    /// Variable lookup rule (kind `"var"`).
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

    /// Object literal rule (kind `"object"`).
    struct Object;
    impl Behavior<Eval> for Object {
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
                    EvalError::TypeMismatch("object: missing \"entries\"".into())
                })?;
            let mut map = serde_json::Map::with_capacity(entries.len());
            for entry in entries {
                let key = entry
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        EvalError::TypeMismatch("object: entry missing \"key\"".into())
                    })?;
                let value_ast = entry.get("value").ok_or_else(|| {
                    EvalError::TypeMismatch("object: entry missing \"value\"".into())
                })?;
                let ast = serde_json::from_value::<AstNode>(value_ast.clone())
                    .map_err(|e| PlatformError::Plugin(e.to_string()))?;
                let value = req.ctx.eval(&ast)?;
                map.insert(key.to_string(), value);
            }
            req.result = Some(Value::Object(map));
            Ok(())
        }
    }
    impl EvalBehavior for Object {}

    fn make_evaluator() -> BasicEvaluator {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(MatchRule) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(WithScope) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueLit) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(Var) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(Object) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
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

    fn with_scope(globals: Value, child: Value) -> Value {
        json!({ "kind": "with_scope", "globals": globals, "child": child })
    }

    fn value_lit(v: Value) -> Value {
        json!({ "kind": "value", "span": null, "payload": { "value": v } })
    }

    fn var_node(name: &str) -> Value {
        json!({ "kind": "var", "span": null, "payload": { "name": name } })
    }

    fn match_node(subject: Value, arms: Vec<Value>) -> Value {
        json!({
            "kind": "match",
            "payload": { "subject": subject, "arms": arms }
        })
    }

    fn arm(pattern: Value, body: Value) -> Value {
        json!({ "pattern": pattern, "guard": Value::Null, "body": body })
    }

    fn arm_with_guard(pattern: Value, guard: Value, body: Value) -> Value {
        json!({ "pattern": pattern, "guard": guard, "body": body })
    }

    // --- literal patterns ---

    #[test]
    fn match_literal_number() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            match_node(
                value_lit(json!(42)),
                vec![
                    arm(value_lit(json!(0)), value_lit(json!("zero"))),
                    arm(value_lit(json!(42)), value_lit(json!("answer"))),
                ],
            ),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("answer"));
    }

    #[test]
    fn match_literal_string() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            match_node(
                value_lit(json!("hi")),
                vec![
                    arm(value_lit(json!("bye")), value_lit(json!(1))),
                    arm(value_lit(json!("hi")), value_lit(json!(2))),
                ],
            ),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn match_no_arm_matches() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            match_node(
                value_lit(json!(99)),
                vec![
                    arm(value_lit(json!(1)), value_lit(json!("a"))),
                    arm(value_lit(json!(2)), value_lit(json!("b"))),
                ],
            ),
        );
        let err = run_eval(&evaluator, payload).unwrap_err();
        assert!(err.to_string().contains("no arm matched"), "got: {err}");
    }

    // --- binder / wildcard ---

    #[test]
    fn match_binder_binds_value() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            match_node(
                value_lit(json!(42)),
                vec![arm(var_node("x"), var_node("x"))],
            ),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn match_wildcard_matches_anything() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            match_node(
                value_lit(json!("anything")),
                vec![arm(var_node("_"), value_lit(json!("matched")))],
            ),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("matched"));
    }

    // --- structural object patterns (AST-traversal case) ---

    #[test]
    fn match_object_pattern_binds_fields() {
        let evaluator = make_evaluator();
        // subject = { "kind": "call", "callee": "f" }
        let subject = json!({
            "kind": "object",
            "span": null,
            "payload": {
                "entries": [
                    { "key": "kind", "value": value_lit(json!("call")) },
                    { "key": "callee", "value": value_lit(json!("f")) }
                ]
            }
        });
        // pattern = { "kind": "call", "callee": g }
        let pattern = json!({
            "kind": "object",
            "span": null,
            "payload": {
                "entries": [
                    { "key": "kind", "value": value_lit(json!("call")) },
                    { "key": "callee", "value": var_node("g") }
                ]
            }
        });
        let payload = with_scope(
            json!({}),
            match_node(subject, vec![arm(pattern, var_node("g"))]),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("f"));
    }

    #[test]
    fn match_object_pattern_kind_discriminator() {
        let evaluator = make_evaluator();
        // subject = { "kind": "call", "payload": { "args": [1, 2] } }
        let subject = json!({
            "kind": "object",
            "span": null,
            "payload": {
                "entries": [
                    { "key": "kind", "value": value_lit(json!("call")) },
                    { "key": "payload", "value": json!({
                        "kind": "object",
                        "span": null,
                        "payload": {
                            "entries": [
                                { "key": "args", "value": value_lit(json!([1, 2])) }
                            ]
                        }
                    }) }
                ]
            }
        });
        // First arm: kind "var" — does not match the "call" discriminator.
        // Second arm: kind "call" — matches, binds the payload.
        let arms = vec![
            arm(
                json!({
                    "kind": "object",
                    "span": null,
                    "payload": {
                        "entries": [
                            { "key": "kind", "value": value_lit(json!("var")) }
                        ]
                    }
                }),
                value_lit(json!("was-var")),
            ),
            arm(
                json!({
                    "kind": "object",
                    "span": null,
                    "payload": {
                        "entries": [
                            { "key": "kind", "value": value_lit(json!("call")) },
                            { "key": "payload", "value": var_node("p") }
                        ]
                    }
                }),
                var_node("p"),
            ),
        ];
        let payload = with_scope(json!({}), match_node(subject, arms));
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!({ "args": [1, 2] }));
    }

    #[test]
    fn match_object_pattern_missing_key_does_not_match() {
        let evaluator = make_evaluator();
        let subject = json!({
            "kind": "object",
            "span": null,
            "payload": {
                "entries": [{ "key": "a", "value": value_lit(json!(1)) }]
            }
        });
        let pattern = json!({
            "kind": "object",
            "span": null,
            "payload": {
                "entries": [
                    { "key": "a", "value": value_lit(json!(1)) },
                    { "key": "b", "value": var_node("x") }
                ]
            }
        });
        let payload = with_scope(
            json!({}),
            match_node(
                subject,
                vec![
                    arm(pattern, value_lit(json!("should-not-match"))),
                    arm(var_node("_"), value_lit(json!("fallback"))),
                ],
            ),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("fallback"));
    }

    // --- guards ---

    #[test]
    fn match_guard_true_selects_arm() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            match_node(
                value_lit(json!(5)),
                vec![arm_with_guard(
                    var_node("x"),
                    // guard: x > 3  — but we only have value/var rules in the
                    // test evaluator, so use a truthy literal.
                    value_lit(json!(true)),
                    var_node("x"),
                )],
            ),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn match_guard_false_skips_arm() {
        let evaluator = make_evaluator();
        let payload = with_scope(
            json!({}),
            match_node(
                value_lit(json!(5)),
                vec![
                    arm_with_guard(
                        var_node("x"),
                        value_lit(json!(false)),
                        value_lit(json!("guarded")),
                    ),
                    arm(var_node("_"), value_lit(json!("fallback"))),
                ],
            ),
        );
        let result = run_eval(&evaluator, payload).unwrap();
        assert_eq!(result, json!("fallback"));
    }

    // --- plugin registration ---

    #[test]
    fn plugin_contributes_rule_once() {
        use handl_platform::DefaultHost;
        let mut host = DefaultHost::default();
        host.register_plugin(BasicEvaluator::new());
        host.register_plugin(EvalMatchPlugin::new());
        host.load().expect("host load should succeed");
        let evaluator = host
            .plugins_mut()
            .get_plugin_mut::<BasicEvaluator>()
            .expect("BasicEvaluator should be registered");
        assert_eq!(evaluator.rules.len(), 1);
    }
}
