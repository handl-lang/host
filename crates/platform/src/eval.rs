//! Generic evaluation operation and `BasicEvaluator` rule acceptor.
//!
//! This module defines the `Eval` operation, `EvalBehavior` trait,
//! and a default `BasicEvaluator` `RuleAcceptor` plugin. The `BasicEvaluator`
//! has no knowledge of concrete language builtins; it dispatches to registered
//! `EvalBehavior` rules keyed by `AstNode.kind`.
//!
//! Unlike other operations, `Eval` carries an [`EvalContext`] inside the
//! [`EvalRequest`]. The context holds variable scopes, resource limits, a
//! call counter, and an [`EvaluatorCallback`] handle that lets recursive
//! rules evaluate child AST nodes by calling `req.ctx.eval(&child)`.
//!
//! Exec-control cross-cutting concerns (limits, tracing, caching, sandboxing)
//! are handled by the lightweight [`ExecControl`] two-hook trait, invoked by
//! `BasicEvaluator` before and after each dispatch.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

use crate::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors specific to evaluation.
///
/// These are raised by `EvalBehavior` rules and exec-control hooks during
/// tree-walking evaluation. They can be converted into [`PlatformError`] via
/// the `From` impl so that `Behavior<Eval>::apply` can return them seamlessly.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum EvalError {
    /// A function was called that is not registered in any module.
    #[error("unknown function: {0}")]
    UnknownFunction(String),
    /// A value did not match the expected type.
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
    /// A variable was referenced that is not bound in any scope.
    #[error("unknown variable: {0}")]
    UnknownVariable(String),
    /// A resource limit (depth, calls, etc.) was exceeded.
    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),
    /// A generic evaluation error.
    #[error("eval error: {0}")]
    Other(String),
}

impl From<EvalError> for PlatformError {
    fn from(e: EvalError) -> Self {
        PlatformError::Plugin(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/// Resource limits for a single evaluation.
///
/// When set, `BasicEvaluator` checks `max_depth` before pushing a new scope
/// and `max_calls` before each dispatch, returning [`EvalError::ResourceLimit`]
/// if the limit would be exceeded.
#[derive(Debug, Clone, Default)]
pub struct EvalLimits {
    /// Maximum scope depth (number of nested scopes).
    pub max_depth: Option<usize>,
    /// Maximum number of dispatch calls.
    pub max_calls: Option<usize>,
}

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/// A variable scope: a map of names to JSON values.
pub type Scope = HashMap<String, Value>;

// ---------------------------------------------------------------------------
// Evaluator callback (recursion handle)
// ---------------------------------------------------------------------------

/// Callback handle that lets an [`EvalContext`] recursively dispatch to the
/// owning `BasicEvaluator`.
///
/// `BasicEvaluator` creates an `Rc<dyn EvaluatorCallback>` at the start of
/// `run` and stores it inside the `EvalContext`. When a rule needs to
/// evaluate a child AST node it calls `ctx.eval(&child)`, which delegates to
/// `EvaluatorCallback::dispatch`.
///
/// The trait is `'static` (no `Send`/`Sync` bound) because evaluation is
/// single-threaded and synchronous, mirroring the existing `CanonicalContext`
/// pattern in `plugin-vm-values-json`.
pub trait EvaluatorCallback: 'static {
    /// Dispatch `req` to the matching `Behavior<Eval>` rule, calling
    /// exec-control hooks before and after. Sets `req.result` on success.
    fn dispatch(&self, req: &mut EvalRequest) -> Result<(), PlatformError>;
}

/// Concrete `EvaluatorCallback` backed by cloned rule and exec-control lists.
///
/// Created by `BasicEvaluator::run`; not intended to be constructed directly
/// by consumer code.
pub struct BasicEvaluatorCallback {
    /// Registered evaluation rules (cloned `Arc` pointers — cheap).
    rules: Vec<Arc<dyn EvalBehavior>>,
    /// Registered exec-control rules.
    exec_controls: Vec<Arc<dyn ExecControl>>,
}

impl EvaluatorCallback for BasicEvaluatorCallback {
    fn dispatch(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        // --- before hooks ---
        for ec in &self.exec_controls {
            ec.before_eval(&req.ast, &mut req.ctx)?;
        }

        // --- dispatch to matching rule(s) ---
        let kind = req.ast.kind.clone();
        let mut found = false;
        for rule in &self.rules {
            if rule.kind() == kind {
                rule.apply(req)?;
                found = true;
            }
        }

        // --- after hooks (only if a result was produced) ---
        if let Some(result) = &req.result {
            for ec in &self.exec_controls {
                ec.after_eval(&req.ast, result, &mut req.ctx)?;
            }
        }

        if !found && req.result.is_none() {
            return Err(PlatformError::NotImplemented);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ExecControl
// ---------------------------------------------------------------------------

/// Lightweight execution-control hook invoked before and after each dispatch.
///
/// Exec-control concerns (limits, tracing, caching, sandboxing) are handled by
/// this two-hook trait rather than a full middleware wrapping trait. Eval is
/// the only operation with cross-cutting execution concerns, so this one
/// deviation from the uniform `Behavior` pattern is justified (see ADR 0040).
///
/// `BasicEvaluator` calls all registered `ExecControl` rules before and after
/// each dispatch. When none are loaded the overhead is a zero-length slice
/// iteration.
pub trait ExecControl: Send + Sync + 'static {
    /// Called before dispatching to the matching `Behavior<Eval>` rule.
    ///
    /// Return `Err` to abort the dispatch.
    fn before_eval(&self, ast: &AstNode, ctx: &mut EvalContext) -> Result<(), PlatformError>;

    /// Called after a successful dispatch, with the produced result.
    ///
    /// Default implementation is a no-op.
    fn after_eval(
        &self,
        _ast: &AstNode,
        _result: &Value,
        _ctx: &mut EvalContext,
    ) -> Result<(), PlatformError> {
        Ok(())
    }
}

/// A collection of [`ExecControl`] rules.
///
/// `BasicEvaluator` holds one of these internally. Kits can add rules via
/// `BasicEvaluator::add_exec_control`.
#[derive(Default)]
pub struct BasicExecControl {
    /// Registered exec-control rules.
    pub rules: Vec<Arc<dyn ExecControl>>,
}

impl BasicExecControl {
    /// Create an empty `BasicExecControl`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an exec-control rule.
    pub fn add(&mut self, rule: Arc<dyn ExecControl>) {
        self.rules.push(rule);
    }
}

// ---------------------------------------------------------------------------
// EvalContext
// ---------------------------------------------------------------------------

/// Execution context carried through an evaluation.
///
/// Holds a stack of variable scopes (JSON values), resource limits, a call
/// counter, and an optional [`EvaluatorCallback`] handle for recursive
/// evaluation. Rules access the context via `req.ctx` and call
/// `req.ctx.eval(&child)` to evaluate sub-expressions.
///
/// The context is the continuation: it captures all mutable state needed to
/// resume or serialize an in-progress evaluation (see ADR 0040 — yield /
/// resumability).
pub struct EvalContext {
    /// Stack of variable scopes, innermost last.
    scopes: Vec<Scope>,
    /// Resource limits for this evaluation.
    limits: EvalLimits,
    /// Number of dispatch calls made so far.
    call_count: usize,
    /// Recursion handle — set by `BasicEvaluator::run`.
    evaluator: Option<Rc<dyn EvaluatorCallback>>,
}

impl EvalContext {
    /// Create a new context with a single empty global scope and no
    /// evaluator handle.
    pub fn new() -> Self {
        EvalContext {
            scopes: vec![HashMap::new()],
            limits: EvalLimits::default(),
            call_count: 0,
            evaluator: None,
        }
    }

    /// Create a context seeded with the given top-level variables.
    pub fn with_globals(mut self, vars: impl IntoIterator<Item = (String, Value)>) -> Self {
        let scope = self.scopes.first_mut().unwrap();
        for (k, v) in vars {
            scope.insert(k, v);
        }
        self
    }

    /// Set the evaluator handle (for recursion).
    pub fn set_evaluator(&mut self, evaluator: Rc<dyn EvaluatorCallback>) {
        self.evaluator = Some(evaluator);
    }

    /// Set resource limits.
    pub fn set_limits(&mut self, limits: EvalLimits) {
        self.limits = limits;
    }

    /// Returns `true` if an evaluator handle is set (recursion is possible).
    pub fn has_evaluator(&self) -> bool {
        self.evaluator.is_some()
    }

    // -- scope management ---------------------------------------------------

    /// Push a new empty scope onto the stack.
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the topmost scope. Returns `None` if only the global scope remains.
    pub fn pop_scope(&mut self) -> Option<Scope> {
        if self.scopes.len() > 1 {
            self.scopes.pop()
        } else {
            None
        }
    }

    /// Current scope depth (global scope is depth 1).
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Define (or overwrite) a variable in the current (topmost) scope.
    pub fn define(&mut self, name: impl Into<String>, value: Value) {
        self.scopes.last_mut().unwrap().insert(name.into(), value);
    }

    /// Look up a variable by name, searching from the innermost scope outward.
    ///
    /// Returns a cloned value if found, or [`EvalError::UnknownVariable`].
    pub fn get(&self, name: &str) -> Result<Value, EvalError> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Ok(value.clone());
            }
        }
        Err(EvalError::UnknownVariable(name.to_string()))
    }

    /// Returns a reference to the variable if found in any scope.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value);
            }
        }
        None
    }

    /// Collect all variables from all scopes into a map.
    ///
    /// Inner scopes shadow outer scopes (later entries overwrite earlier
    /// ones). Used by function-literal evaluation to capture the closure
    /// environment.
    pub fn collect_vars(&self) -> HashMap<String, Value> {
        let mut vars = HashMap::new();
        for scope in &self.scopes {
            for (name, value) in scope {
                vars.insert(name.clone(), value.clone());
            }
        }
        vars
    }

    // -- limits / counters --------------------------------------------------

    /// Returns the configured limits.
    pub fn limits(&self) -> &EvalLimits {
        &self.limits
    }

    /// Returns the number of dispatch calls made so far.
    pub fn call_count(&self) -> usize {
        self.call_count
    }

    /// Increment the call counter. Returns an error if `max_calls` is exceeded.
    pub fn increment_call_count(&mut self) -> Result<(), PlatformError> {
        self.call_count += 1;
        if let Some(max) = self.limits.max_calls {
            if self.call_count > max {
                return Err(EvalError::ResourceLimit(format!(
                    "call limit exceeded: {max}"
                ))
                .into());
            }
        }
        Ok(())
    }

    /// Check whether pushing another scope would exceed `max_depth`.
    pub fn check_depth(&self) -> Result<(), PlatformError> {
        if let Some(max) = self.limits.max_depth {
            if self.scopes.len() >= max {
                return Err(EvalError::ResourceLimit(format!(
                    "depth limit exceeded: {max}"
                ))
                .into());
            }
        }
        Ok(())
    }

    // -- recursive evaluation -----------------------------------------------

    /// Evaluate a child AST node by dispatching through the evaluator handle.
    ///
    /// This is the primary recursion mechanism for non-leaf `EvalBehavior`
    /// rules. A handler extracts a child node from its `AstNode.payload`,
    /// reconstructs an `AstNode { kind, payload }`, and calls
    /// `ctx.eval(&child)`.
    ///
    /// Implementation note: the context is temporarily moved out of `self`
    /// into a fresh `EvalRequest` via `std::mem::take`, dispatched, and then
    /// restored. This avoids self-referential borrow issues while keeping
    /// `EvalRequest` an owned (lifetime-free) struct.
    pub fn eval(&mut self, ast: &AstNode) -> Result<Value, PlatformError> {
        let evaluator = self
            .evaluator
            .clone()
            .ok_or(PlatformError::NotImplemented)?;

        // Temporarily move the context out into a fresh request.
        let ctx = std::mem::take(self);
        let mut req = EvalRequest {
            ast: ast.clone(),
            ctx,
            options: ResolvedOptions::new(),
            result: None,
        };

        let dispatch_result = evaluator.dispatch(&mut req);

        // Always restore the context, even on error.
        *self = req.ctx;

        dispatch_result?;
        req.result.ok_or(PlatformError::NotImplemented)
    }
}

impl Default for EvalContext {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EvalContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvalContext")
            .field("scopes", &self.scopes)
            .field("limits", &self.limits)
            .field("call_count", &self.call_count)
            .field("has_evaluator", &self.evaluator.is_some())
            .finish()
    }
}

impl Clone for EvalContext {
    fn clone(&self) -> Self {
        EvalContext {
            scopes: self.scopes.clone(),
            limits: self.limits.clone(),
            call_count: self.call_count,
            evaluator: self.evaluator.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Eval operation & request
// ---------------------------------------------------------------------------

/// Eval operation marker.
#[derive(Debug, Clone, Copy)]
pub struct Eval;

impl Operation for Eval {
    type Request = EvalRequest;
}

/// Request passed to `Eval` behaviors.
///
/// Carries the AST node to evaluate, an [`EvalContext`] with variable scopes
/// and a recursion handle, resolved options, and an optional result slot.
/// `Behavior<Eval>::apply(&self, req: &mut EvalRequest)` reads `req.ast` and
/// `req.ctx`, recurses via `req.ctx.eval(&child)`, and writes `req.result`.
pub struct EvalRequest {
    /// The AST node to evaluate.
    pub ast: AstNode,
    /// Execution context (scopes, limits, recursion handle).
    pub ctx: EvalContext,
    /// Resolved evaluator options.
    pub options: ResolvedOptions,
    /// Optional result produced by a behavior.
    pub result: Option<Value>,
}

impl std::fmt::Debug for EvalRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvalRequest")
            .field("ast", &self.ast)
            .field("ctx", &self.ctx)
            .field("options", &self.options)
            .field("result", &self.result)
            .finish()
    }
}

impl Clone for EvalRequest {
    fn clone(&self) -> Self {
        EvalRequest {
            ast: self.ast.clone(),
            ctx: self.ctx.clone(),
            options: self.options.clone(),
            result: self.result.clone(),
        }
    }
}

/// Behavior trait for evaluating a specific `AstNode.kind`.
pub trait EvalBehavior: Behavior<Eval> {}

impl Contribution for Arc<dyn EvalBehavior> {
    fn id(&self) -> &str {
        self.kind()
    }
}

// ---------------------------------------------------------------------------
// BasicEvaluator
// ---------------------------------------------------------------------------

/// Basic `RuleAcceptor` plugin for the `Eval` operation.
///
/// Holds registered `EvalBehavior` rules (keyed by `AstNode.kind` at dispatch
/// time) and `ExecControl` hooks. `run` creates an `EvalContext` with a
/// recursion handle, wraps it in an `EvalRequest`, and dispatches.
pub struct BasicEvaluator {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered evaluation rules, keyed by `AstNode.kind`.
    pub rules: Vec<Arc<dyn EvalBehavior>>,
    /// Registered exec-control rules.
    pub exec_controls: Vec<Arc<dyn ExecControl>>,
    /// Option schemas contributed for the evaluator.
    pub option_schemas: Vec<OptionSchema>,
}

impl BasicEvaluator {
    /// Create a new `BasicEvaluator` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("eval")],
            rules: Vec::new(),
            exec_controls: Vec::new(),
            option_schemas: Vec::new(),
        }
    }

    /// Add an exec-control rule.
    pub fn add_exec_control(&mut self, rule: Arc<dyn ExecControl>) {
        self.exec_controls.push(rule);
    }
}

impl Default for BasicEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BasicEvaluator {
    fn id(&self) -> &str {
        "eval.basic"
    }

    fn name(&self) -> &str {
        "Basic evaluator"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "eval").then(|| {
            serde_json::json!({
                "command": "eval",
                "description": "Evaluate a Handl expression"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if !self.capabilities.contains(&request.capability) {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        let kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("eval");

        // Build the recursion handle (clones Arc pointers — cheap).
        let callback = Rc::new(BasicEvaluatorCallback {
            rules: self.rules.clone(),
            exec_controls: self.exec_controls.clone(),
        });

        // Build the context and wire up the handle.
        let mut ctx = EvalContext::new();
        ctx.set_evaluator(callback.clone());

        let mut req = EvalRequest {
            ast: AstNode {
                kind: kind.to_string(),
                span: None,
                payload: request.payload.clone(),
            },
            ctx,
            options: ResolvedOptions::new(),
            result: None,
        };

        callback.dispatch(&mut req)?;

        let result = req.result.ok_or(PlatformError::NotImplemented)?;
        Ok(CapabilityResponse(result))
    }
}

impl RuleAcceptor for BasicEvaluator {
    type Rule = Arc<dyn EvalBehavior>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KitRegistry, PluginRegistry};

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = BasicEvaluator::new();
        let meta = plugin.metadata(&CapabilityId::from("eval")).unwrap();
        assert_eq!(meta["command"], "eval");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_evaluated_result_when_rule_matches() {
        struct Echo;

        impl Behavior<Eval> for Echo {
            fn kind(&self) -> &str {
                "echo"
            }

            fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
                op.result = Some(op.ast.payload.clone());
                Ok(())
            }
        }

        impl EvalBehavior for Echo {}

        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(Echo) as Arc<dyn EvalBehavior>)
            .unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"kind": "echo", "value": 42}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!({"kind": "echo", "value": 42}));
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicEvaluator::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }

    #[test]
    fn eval_context_scope_management() {
        let mut ctx = EvalContext::new();
        assert_eq!(ctx.depth(), 1);
        ctx.define("x", serde_json::json!(42));
        assert_eq!(ctx.lookup("x"), Some(&serde_json::json!(42)));

        ctx.push_scope();
        ctx.define("y", serde_json::json!("hello"));
        assert_eq!(ctx.depth(), 2);
        // inner scope can see outer
        assert_eq!(ctx.lookup("x"), Some(&serde_json::json!(42)));
        assert_eq!(ctx.lookup("y"), Some(&serde_json::json!("hello")));

        // shadowing
        ctx.define("x", serde_json::json!(99));
        assert_eq!(ctx.lookup("x"), Some(&serde_json::json!(99)));

        ctx.pop_scope();
        assert_eq!(ctx.depth(), 1);
        assert_eq!(ctx.lookup("x"), Some(&serde_json::json!(42)));
        assert!(ctx.lookup("y").is_none());
    }

    #[test]
    fn eval_context_get_returns_unknown_variable_error() {
        let ctx = EvalContext::new();
        let err = ctx.get("missing").unwrap_err();
        assert_eq!(err, EvalError::UnknownVariable("missing".to_string()));
    }

    #[test]
    fn eval_context_increment_call_count_enforces_limit() {
        let mut ctx = EvalContext::new();
        ctx.set_limits(EvalLimits {
            max_calls: Some(2),
            max_depth: None,
        });
        assert!(ctx.increment_call_count().is_ok()); // count = 1
        assert!(ctx.increment_call_count().is_ok()); // count = 2
        assert!(ctx.increment_call_count().is_err()); // count = 3 > 2
    }

    #[test]
    fn eval_context_check_depth_enforces_limit() {
        let mut ctx = EvalContext::new();
        ctx.set_limits(EvalLimits {
            max_calls: None,
            max_depth: Some(2),
        });
        assert!(ctx.check_depth().is_ok()); // depth = 1 < 2
        ctx.push_scope();
        assert_eq!(ctx.depth(), 2);
        assert!(ctx.check_depth().is_err()); // depth = 2 >= 2
    }

    #[test]
    fn eval_context_eval_without_handle_returns_not_implemented() {
        let mut ctx = EvalContext::new();
        let ast = AstNode {
            kind: "test".to_string(),
            span: None,
            payload: serde_json::Value::Null,
        };
        let err = ctx.eval(&ast).unwrap_err();
        assert!(matches!(err, PlatformError::NotImplemented));
    }

    #[test]
    fn recursive_eval_dispatches_to_child_rule() {
        // A "double" rule that evaluates its child and doubles the result.
        struct Double;

        impl Behavior<Eval> for Double {
            fn kind(&self) -> &str {
                "double"
            }

            fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
                // Extract the child AST from payload.
                let child_kind = op
                    .ast
                    .payload
                    .get("child_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("value");
                let child_payload = op
                    .ast
                    .payload
                    .get("child_payload")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let child = AstNode {
                    kind: child_kind.to_string(),
                    span: None,
                    payload: child_payload,
                };
                let inner = op.ctx.eval(&child)?;
                if let Some(n) = inner.as_i64() {
                    op.result = Some(serde_json::json!(n * 2));
                } else {
                    op.result = Some(inner);
                }
                Ok(())
            }
        }
        impl EvalBehavior for Double {}

        // A leaf "value" rule that returns the payload's "value" field.
        struct ValueRule;
        impl Behavior<Eval> for ValueRule {
            fn kind(&self) -> &str {
                "value"
            }
            fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
                op.result = Some(
                    op.ast
                        .payload
                        .get("value")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                );
                Ok(())
            }
        }
        impl EvalBehavior for ValueRule {}

        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(Double) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.accept(Arc::new(ValueRule) as Arc<dyn EvalBehavior>).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({
                        "kind": "double",
                        "child_kind": "value",
                        "child_payload": { "value": 21 }
                    }),
                },
                &mut pctx,
            )
            .unwrap();
        assert_eq!(resp.0, serde_json::json!(42));
    }

    #[test]
    fn exec_control_hooks_are_called() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingControl {
            before: Arc<AtomicUsize>,
            after: Arc<AtomicUsize>,
        }

        impl ExecControl for CountingControl {
            fn before_eval(&self, _ast: &AstNode, _ctx: &mut EvalContext) -> Result<(), PlatformError> {
                self.before.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            fn after_eval(
                &self,
                _ast: &AstNode,
                _result: &Value,
                _ctx: &mut EvalContext,
            ) -> Result<(), PlatformError> {
                self.after.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        struct Echo;
        impl Behavior<Eval> for Echo {
            fn kind(&self) -> &str {
                "echo"
            }
            fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
                op.result = Some(op.ast.payload.clone());
                Ok(())
            }
        }
        impl EvalBehavior for Echo {}

        let before = Arc::new(AtomicUsize::new(0));
        let after = Arc::new(AtomicUsize::new(0));

        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(Echo) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.add_exec_control(Arc::new(CountingControl {
            before: before.clone(),
            after: after.clone(),
        }));

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let _ = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"kind": "echo"}),
                },
                &mut pctx,
            )
            .unwrap();

        assert_eq!(before.load(Ordering::SeqCst), 1);
        assert_eq!(after.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn exec_control_before_eval_can_abort() {
        struct DenyAll;
        impl ExecControl for DenyAll {
            fn before_eval(&self, _ast: &AstNode, _ctx: &mut EvalContext) -> Result<(), PlatformError> {
                Err(EvalError::ResourceLimit("denied".into()).into())
            }
        }

        struct Echo;
        impl Behavior<Eval> for Echo {
            fn kind(&self) -> &str {
                "echo"
            }
            fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
                op.result = Some(op.ast.payload.clone());
                Ok(())
            }
        }
        impl EvalBehavior for Echo {}

        let mut evaluator = BasicEvaluator::new();
        evaluator.accept(Arc::new(Echo) as Arc<dyn EvalBehavior>).unwrap();
        evaluator.add_exec_control(Arc::new(DenyAll));

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"kind": "echo"}),
                },
                &mut pctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::Plugin(_)));
    }

    #[test]
    fn eval_error_converts_to_platform_error() {
        let err = EvalError::UnknownFunction("foo".into());
        let pe: PlatformError = err.into();
        assert!(matches!(pe, PlatformError::Plugin(_)));
    }
}
