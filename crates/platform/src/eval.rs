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

/// String-expression evaluator callback for host functions that need to
/// evaluate expression strings (e.g. `FunctionValue.body`) with variable
/// bindings.
///
/// This is the ADR 0040 replacement for the old `Eval<V>` trait from
/// `plugin-vm`. The concrete implementation is provided by the host
/// (e.g. `kit-canonical`) and set on `EvalContext` via
/// [`EvalContext::set_expr_evaluator`].
pub trait ExprEvaluator: 'static {
    /// Evaluate an expression string with the given variable bindings.
    ///
    /// `expr` is a raw expression string (e.g. `"x"`, `"42"`, `"add(x, 1)"`).
    /// `vars` are `(name, value)` pairs to bind before evaluation.
    fn eval_expr(
        &self,
        expr: &str,
        vars: &[(String, Value)],
    ) -> Result<Value, EvalError>;
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
        // A `ControlFlow::Skip(value)` from any hook short-circuits dispatch:
        // the value becomes the result and no `Behavior<Eval>` rule runs.
        // After-hooks still fire so observability rules (trace, counter) see
        // the call, and the cache rule can (re)record the result.
        let mut skipped = false;
        for ec in &self.exec_controls {
            match ec.before_eval(&mut req.ast, &mut req.ctx)? {
                ControlFlow::Continue => {}
                ControlFlow::Skip(value) => {
                    req.result = Some(value);
                    skipped = true;
                    break;
                }
            }
        }

        if !skipped {
            // --- dispatch to matching rule(s) ---
            let kind = req.ast.kind.clone();
            let mut found = false;
            for rule in &self.rules {
                if rule.kind() == kind {
                    rule.apply(req)?;
                    found = true;
                }
            }

            if !found && req.result.is_none() {
                return Err(PlatformError::NotImplemented);
            }
        }

        // --- after hooks (only if a result was produced) ---
        if let Some(result) = &req.result {
            for ec in &self.exec_controls {
                ec.after_eval(&req.ast, result, &mut req.ctx)?;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ExecControl
// ---------------------------------------------------------------------------

/// Control flow outcome from an [`ExecControl::before_eval`] hook.
///
/// `Continue` lets dispatch proceed to the matching `Behavior<Eval>` rule.
/// `Skip(value)` short-circuits dispatch: `value` becomes the result and no
/// behavior rule runs (after-hooks still fire). This is how the cache plugin
/// returns a cached result without re-executing.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlFlow {
    /// Proceed with normal dispatch.
    Continue,
    /// Skip dispatch and use `value` as the result.
    Skip(Value),
}

impl ControlFlow {
    /// Convenience constructor for the `Continue` variant.
    pub fn continue_() -> Self {
        ControlFlow::Continue
    }
}

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
///
/// `before_eval` receives a `&mut AstNode` so that rewriting rules (e.g. the
/// map plugin) can mutate the node in place before dispatch. Returning
/// [`ControlFlow::Skip`] short-circuits dispatch with a precomputed result
/// (used by the cache plugin).
pub trait ExecControl: Send + Sync + 'static {
    /// Called before dispatching to the matching `Behavior<Eval>` rule.
    ///
    /// Return `Err` to abort the dispatch, [`ControlFlow::Skip`] to
    /// short-circuit with a cached result, or [`ControlFlow::Continue`] to
    /// proceed.
    fn before_eval(
        &self,
        ast: &mut AstNode,
        ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError>;

    /// Called after a successful dispatch (or a short-circuited skip), with
    /// the produced result.
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
// Function / Module / ModuleLookup traits
// ---------------------------------------------------------------------------

/// A callable host function, non-generic per ADR 0040.
///
/// Functions live in modules and are looked up by qualified name
/// (e.g. `math.add`) via [`ModuleLookup`]. The `call` method receives the
/// already-evaluated JSON arguments and a mutable [`EvalContext`] so the
/// function can itself recurse (`ctx.eval`, `ctx.call_function`) or inspect
/// variable bindings. There is no `V` genericity — values are
/// `serde_json::Value`.
pub trait Function: Send + Sync {
    /// Invoke the function.
    ///
    /// `args` are the already-evaluated argument values. `ctx` is the
    /// evaluation context the call is happening within.
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError>;
}

/// A named group of functions — the runtime-side analogue of a namespace.
///
/// A module provides a namespace for host functions. The module name becomes
/// the first segment of a qualified function name — e.g. a module named
/// `"math"` providing a function `"add"` is callable as `math.add(1, 2)`.
/// Which modules exist is a runtime host decision, not a language-level
/// feature.
pub trait Module: Send + Sync {
    /// The module name — the namespace prefix for qualified calls.
    fn name(&self) -> &str;

    /// Resolve a function name (without the module prefix) to a callable.
    ///
    /// Returns `None` if the name is not recognized in this module.
    fn function(&self, name: &str) -> Option<&dyn Function>;

    /// Enumerate all `(name, function)` pairs in this module.
    ///
    /// The default implementation is empty; modules override it to expose
    /// their full function table (used for introspection and testing).
    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        Vec::new()
    }
}

/// Lookup handle for qualified function names, held by [`EvalContext`].
///
/// `EvalContext` cannot depend on `plugin-module` (where the concrete
/// [`ModuleRegistry`](../handl_plugin_module/struct.ModuleRegistry.html)
/// lives), so it holds an `Option<Rc<dyn ModuleLookup>>`. The concrete
/// `ModuleRegistry` in `plugin-module` implements this trait. When no
/// registry is wired up, `call_function` returns
/// [`EvalError::UnknownFunction`].
pub trait ModuleLookup: 'static {
    /// Resolve a qualified name (e.g. `math.add`) or flat name (e.g. `add`)
    /// to a callable [`Function`].
    fn lookup(&self, qualified_name: &str) -> Option<&dyn Function>;
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
    /// Module/function lookup handle — set by the host before evaluation
    /// so eval rules can call `ctx.call_function("math.add", &[1, 2])`.
    modules: Option<Rc<dyn ModuleLookup>>,
    /// String-expression evaluator — set by the host so host functions
    /// can call `ctx.eval_expr("x", &[("x", value)])` to evaluate
    /// expression strings from `FunctionValue.body`.
    expr_evaluator: Option<Rc<dyn ExprEvaluator>>,
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
            modules: None,
            expr_evaluator: None,
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

    /// Set the module/function lookup handle (for `call_function`).
    ///
    /// The host wires a [`ModuleLookup`] (typically a `ModuleRegistry` from
    /// `plugin-module`) into the context before evaluation so eval rules can
    /// call registered host functions by qualified name.
    pub fn set_modules(&mut self, modules: Rc<dyn ModuleLookup>) {
        self.modules = Some(modules);
    }

    /// Returns `true` if a module lookup handle is set.
    pub fn has_modules(&self) -> bool {
        self.modules.is_some()
    }

    /// Set the string-expression evaluator handle.
    ///
    /// The host wires an [`ExprEvaluator`] into the context so host
    /// functions (e.g. `fp.map`, `option.map`) can evaluate expression
    /// strings from `FunctionValue.body` via `ctx.eval_expr`.
    pub fn set_expr_evaluator(&mut self, evaluator: Rc<dyn ExprEvaluator>) {
        self.expr_evaluator = Some(evaluator);
    }

    /// Evaluate an expression string with the given variable bindings.
    ///
    /// Delegates to the [`ExprEvaluator`] set via [`set_expr_evaluator`].
    /// Returns [`EvalError::Other`] if no evaluator is set.
    pub fn eval_expr(
        &mut self,
        expr: &str,
        vars: &[(String, Value)],
    ) -> Result<Value, EvalError> {
        let evaluator = self
            .expr_evaluator
            .clone()
            .ok_or_else(|| EvalError::Other("no expr evaluator set".to_string()))?;
        evaluator.eval_expr(expr, vars)
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

    /// Increment the call counter and return the new count.
    ///
    /// Unlike [`increment_call_count`](Self::increment_call_count), this does
    /// **not** check `limits.max_calls` — the enforcing rule (e.g.
    /// `plugin-exec-max-calls`) checks the returned count against its own
    /// configured maximum.
    pub fn bump_call_count(&mut self) -> usize {
        self.call_count += 1;
        self.call_count
    }

    /// Increment the call counter. Returns an error if `max_calls` is exceeded.
    pub fn increment_call_count(&mut self) -> Result<(), PlatformError> {
        self.call_count += 1;
        if let Some(max) = self.limits.max_calls {
            if self.call_count > max {
                return Err(EvalError::ResourceLimit(format!("call limit exceeded: {max}")).into());
            }
        }
        Ok(())
    }

    /// Check whether pushing another scope would exceed `max_depth`.
    pub fn check_depth(&self) -> Result<(), PlatformError> {
        if let Some(max) = self.limits.max_depth {
            if self.scopes.len() >= max {
                return Err(
                    EvalError::ResourceLimit(format!("depth limit exceeded: {max}")).into(),
                );
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

    /// Call a registered host function by qualified name.
    ///
    /// Looks up `name` (e.g. `math.add` or a flat `add`) through the
    /// [`ModuleLookup`] handle wired via [`set_modules`]. The arguments are
    /// passed as already-evaluated JSON values. Returns
    /// [`EvalError::UnknownFunction`] if no module registry is set or the
    /// name is not found.
    ///
    /// This is the bridge between the eval tree-walker and the
    /// `Module`/`Function` registration mechanism (see ADR 0040).
    pub fn call_function(&mut self, name: &str, args: &[Value]) -> Result<Value, EvalError> {
        // Clone the Rc so we can borrow the lookup without borrowing `self`,
        // then call the function with `&mut self` as the context.
        let modules = self
            .modules
            .clone()
            .ok_or_else(|| EvalError::UnknownFunction(name.to_string()))?;
        let func = modules
            .lookup(name)
            .ok_or_else(|| EvalError::UnknownFunction(name.to_string()))?;
        func.call(args, self)
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
            .field("has_modules", &self.modules.is_some())
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
            modules: self.modules.clone(),
            expr_evaluator: self.expr_evaluator.clone(),
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

/// `Contribution` impl for exec-control rules so they can be deferred through
/// the pending-rule mechanism. The id is a stable placeholder; exec-control
/// rules are not keyed by kind (they apply to every dispatch).
impl Contribution for Arc<dyn ExecControl> {
    fn id(&self) -> &str {
        "exec-control"
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
        evaluator
            .accept(Arc::new(Double) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ValueRule) as Arc<dyn EvalBehavior>)
            .unwrap();

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
            fn before_eval(
                &self,
                _ast: &mut AstNode,
                _ctx: &mut EvalContext,
            ) -> Result<ControlFlow, PlatformError> {
                self.before.fetch_add(1, Ordering::SeqCst);
                Ok(ControlFlow::Continue)
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
        evaluator
            .accept(Arc::new(Echo) as Arc<dyn EvalBehavior>)
            .unwrap();
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
            fn before_eval(
                &self,
                _ast: &mut AstNode,
                _ctx: &mut EvalContext,
            ) -> Result<ControlFlow, PlatformError> {
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
        evaluator
            .accept(Arc::new(Echo) as Arc<dyn EvalBehavior>)
            .unwrap();
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
