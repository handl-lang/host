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
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
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
    /// Execution yielded, capturing the current [`EvalContext`] for later
    /// resumption.
    ///
    /// The context *is* the continuation (see ADR 0040 — yield / resumability).
    /// A rule returns this variant to suspend evaluation mid-flight; the host
    /// catches the resulting [`PlatformError::Yield`], serializes the context,
    /// and later resumes by re-dispatching `Eval` with a restored context.
    #[error("evaluation yielded")]
    Yield {
        /// The captured execution context — all state needed to resume.
        context: EvalContext,
    },
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
        match e {
            EvalError::Yield { context } => PlatformError::Yield { context },
            other => PlatformError::Plugin(other.to_string()),
        }
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
/// single-threaded and synchronous.
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
    fn eval_expr(&self, expr: &str, vars: &[(String, Value)]) -> Result<Value, EvalError>;
}

/// A callable value evaluator — allows host functions to call arrow-values
/// (first-class function values from the tree-walk engine) by passing the
/// value and arguments through the eval engine.
///
/// This is the bridge between `fn.*` combinators (which receive arrow-values
/// as arguments) and the tree-walk engine (which knows how to invoke them).
pub trait ValueCallable: 'static {
    /// Call a callable value (e.g. arrow-value `{__arrow__: true, ...}`) with
    /// the given arguments. Returns the result value.
    fn call_value(&self, callee: &Value, args: &[Value]) -> Result<Value, EvalError>;
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
pub trait Function: 'static {
    /// Invoke the function.
    ///
    /// `args` are the already-evaluated argument values. `ctx` is the
    /// evaluation context the call is happening within.
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError>;

    /// Downcast this function to an [`AsyncFunction`] if it supports async
    /// execution.
    ///
    /// The default implementation returns `None` — sync-only functions do
    /// nothing here. Functions that also implement [`AsyncFunction`] override
    /// this to return `Some(self)` so that [`EvalContext::call_function_async`]
    /// can dispatch through the async path.
    ///
    /// This is the bridge between the sync `Function` trait object held by
    /// [`ModuleLookup`] and the async call path: lookup returns a
    /// `&dyn Function`, and `call_function_async` uses `as_async` to decide
    /// whether to await or call sync.
    fn as_async(&self) -> Option<&dyn AsyncFunction> {
        None
    }

    /// Downcast this function to a [`TypeSignature`] if it exposes a type
    /// signature for typechecking (ADR-0083).
    ///
    /// The default implementation returns `None`. Functions that also
    /// implement [`TypeSignature`] override this to return `Some(self)` so
    /// that [`ModuleLookup::lookup_type_signature`] can retrieve the op's
    /// declared type without executing it.
    fn as_type_signature(&self) -> Option<&dyn TypeSignature> {
        None
    }
}

/// A function that exposes its type signature for typechecking (ADR-0083).
///
/// `Function` impls that represent declared `op`s (with `op "x" : T -> T`
/// signatures) implement this trait so the typechecker can look up the op's
/// type without executing it. The signature is a type value (ADR-0032).
pub trait TypeSignature: Function {
    /// The op's type signature as a type value (e.g. a `type.function` value).
    fn type_signature(&self) -> Value;
}

/// A callable host function that supports async execution.
///
/// This is the async counterpart of [`Function`], per ADR 0040's "Async
/// evaluation" section. Functions that perform IO-bound work (HTTP, WebSocket,
/// timers, sockets) implement this trait so they can be awaited from
/// [`EvalContext::call_function_async`] without blocking the calling thread.
///
/// A type can implement *both* [`Function`] and [`AsyncFunction`]: the sync
/// `call` is used by [`EvalContext::call_function`], and `call_async` is used
/// by [`EvalContext::call_function_async`]. To make the async path visible to
/// the lookup machinery, the [`Function::as_async`] override must return
/// `Some(self)`.
///
/// The returned future is `Pin<Box<dyn Future + 'a>>` — single-threaded
/// (consistent with [`EvalContext`]'s `Rc`-based handles), no `Send` bound.
pub trait AsyncFunction: 'static {
    /// Invoke the function asynchronously.
    ///
    /// `args` are the already-evaluated argument values. `ctx` is the
    /// evaluation context the call is happening within. The returned future
    /// borrows `self`, `args`, and `ctx` for its lifetime `'a`.
    fn call_async<'a>(
        &'a self,
        args: &'a [Value],
        ctx: &'a mut EvalContext,
    ) -> Pin<Box<dyn Future<Output = Result<Value, EvalError>> + 'a>>;
}

/// A named group of functions — the runtime-side analogue of a namespace.
///
/// A module provides a namespace for host functions. The module name becomes
/// the first segment of a qualified function name — e.g. a module named
/// `"math"` providing a function `"add"` is callable as `math.add(1, 2)`.
/// Which modules exist is a runtime host decision, not a language-level
/// feature.
pub trait Module: 'static {
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

    /// Look up the type signature of a qualified op name (ADR-0083).
    ///
    /// Returns `None` if the op is not found or does not expose a type
    /// signature (i.e. its [`Function`] impl does not also implement
    /// [`TypeSignature`]).
    fn lookup_type_signature(&self, qualified_name: &str) -> Option<Value> {
        let func = self.lookup(qualified_name)?;
        func.as_type_signature().map(|ts| ts.type_signature())
    }
}

// ---------------------------------------------------------------------------
// HandlerClause (effect handlers)
// ---------------------------------------------------------------------------

/// A handler clause in a `handle` expression.
///
/// When an operation (effect) is performed inside a `handle` expression, the
/// matching handler clause runs. If the handler resumes (by calling the
/// continuation), the computation continues from where the effect was
/// performed.
///
/// See ADR 0040 — Continuation / yield / resumability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandlerClause {
    /// The operation name this clause matches (e.g. `"fetch"`).
    pub operation: String,
    /// Parameter names bound when the handler fires (bound to the
    /// operation's evaluated arguments).
    pub params: Vec<String>,
    /// The continuation name — bound to a resume function in the handler's
    /// scope so the handler body can call `resume(value)` to continue the
    /// computation that performed the effect.
    pub resume_name: String,
    /// The handler body AST. Evaluated when the handler fires; its result
    /// replaces the operation's result in the surrounding computation.
    pub body: AstNode,
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
#[derive(Serialize, Deserialize)]
pub struct EvalContext {
    /// Stack of variable scopes, innermost last.
    scopes: Vec<Scope>,
    /// Resource limits for this evaluation.
    limits: EvalLimits,
    /// Number of dispatch calls made so far.
    call_count: usize,
    /// Recursion handle — set by `BasicEvaluator::run`.
    ///
    /// Runtime-only; not serialized. On deserialization this is `None` and
    /// must be re-attached by the host when resuming.
    #[serde(skip)]
    evaluator: Option<Rc<dyn EvaluatorCallback>>,
    /// Module/function lookup handle — set by the host before evaluation
    /// so eval rules can call `ctx.call_function("math.add", &[1, 2])`.
    ///
    /// Runtime-only; not serialized. Re-attached by the host on resume.
    #[serde(skip)]
    modules: Option<Rc<dyn ModuleLookup>>,
    /// String-expression evaluator — set by the host so host functions
    /// can call `ctx.eval_expr("x", &[("x", value)])` to evaluate
    /// expression strings from `FunctionValue.body`.
    ///
    /// Runtime-only; not serialized. Re-attached by the host on resume.
    #[serde(skip)]
    expr_evaluator: Option<Rc<dyn ExprEvaluator>>,
    /// Value callable — set by the host so host functions can call
    /// arrow-values (first-class function values) via `ctx.call_value`.
    ///
    /// Runtime-only; not serialized. Re-attached by the host on resume.
    #[serde(skip)]
    value_callable: Option<Rc<dyn ValueCallable>>,
    /// Stack of handler frames from enclosing `handle` expressions.
    ///
    /// Each frame is a list of [`HandlerClause`]s from one `handle` expression.
    /// Frames are pushed outermost-first, so `handlers[0]` is the outermost
    /// (highest-precedence) frame. When an operation is performed, the call
    /// rule searches from the outermost frame inward — outer handlers take
    /// precedence over inner handlers, matching the original interpreter's
    /// merge-and-reverse search order.
    ///
    /// Serializable: handler clauses contain only AST data (no runtime
    /// handles), so the handler stack is part of the continuation state.
    handlers: Vec<Vec<HandlerClause>>,
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
            value_callable: None,
            handlers: Vec::new(),
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

    /// Set the value callable handle (for `call_value`).
    ///
    /// The host wires a [`ValueCallable`] (typically the tree-walk engine)
    /// into the context so host functions like `fn.map` can call arrow-values
    /// passed as arguments.
    pub fn set_value_callable(&mut self, callable: Rc<dyn ValueCallable>) {
        self.value_callable = Some(callable);
    }

    /// Call a callable value (e.g. arrow-value) with the given arguments.
    ///
    /// Delegates to the [`ValueCallable`] set via [`set_value_callable`].
    /// Returns `None` if no value callable is set (caller can fall back
    /// to `FunctionValue` / `eval_expr`).
    pub fn try_call_value(
        &mut self,
        callee: &Value,
        args: &[Value],
    ) -> Option<Result<Value, EvalError>> {
        let callable = self.value_callable.clone()?;
        Some(callable.call_value(callee, args))
    }

    /// Evaluate an expression string with the given variable bindings.
    ///
    /// Delegates to the [`ExprEvaluator`] set via [`set_expr_evaluator`].
    /// Returns [`EvalError::Other`] if no evaluator is set.
    pub fn eval_expr(&mut self, expr: &str, vars: &[(String, Value)]) -> Result<Value, EvalError> {
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

    /// Asynchronously evaluate a child AST node.
    ///
    /// This is the async counterpart of [`eval`](Self::eval), per ADR 0040's
    /// "Async evaluation" section. The simplest viable approach is used:
    /// dispatch goes through the *same* sync [`EvaluatorCallback`] (sync
    /// `Behavior<Eval>` rules work unchanged) and the result is wrapped in a
    /// `Future`. Sync rules need no async impl; only [`Function`] impls that
    /// need IO-bound work implement [`AsyncFunction`] and are reached via
    /// [`call_function_async`](Self::call_function_async).
    ///
    /// The platform's `Plugin::run` contract stays sync — async happens
    /// *inside* evaluation, not at the capability dispatch boundary.
    pub fn eval_async<'a>(
        &'a mut self,
        node: &'a AstNode,
    ) -> Pin<Box<dyn Future<Output = Result<Value, PlatformError>> + 'a>> {
        Box::pin(async move { self.eval(node) })
    }

    /// Asynchronously call a registered host function by qualified name.
    ///
    /// Looks up `name` through the [`ModuleLookup`] handle (same as
    /// [`call_function`](Self::call_function)). If the resolved function
    /// also implements [`AsyncFunction`] (advertised via
    /// [`Function::as_async`]), it is awaited through `call_async`; otherwise
    /// the sync `call` is invoked and its result is wrapped in a ready
    /// future. This lets async-capable functions (HTTP, WebSocket, timers)
    /// coexist with sync functions in the same registry.
    ///
    /// Returns [`EvalError::UnknownFunction`] if no module registry is set or
    /// the name is not found.
    pub fn call_function_async<'a>(
        &'a mut self,
        name: &'a str,
        args: &'a [Value],
    ) -> Pin<Box<dyn Future<Output = Result<Value, EvalError>> + 'a>> {
        Box::pin(async move {
            let modules = self
                .modules
                .clone()
                .ok_or_else(|| EvalError::UnknownFunction(name.to_string()))?;
            let func = modules
                .lookup(name)
                .ok_or_else(|| EvalError::UnknownFunction(name.to_string()))?;
            if let Some(async_fn) = func.as_async() {
                async_fn.call_async(args, self).await
            } else {
                func.call(args, self)
            }
        })
    }

    // -- yield / resumability -----------------------------------------------

    /// Yield execution, capturing the current context for later resumption.
    ///
    /// A rule calls this (after taking ownership of the context via
    /// `std::mem::take(&mut req.ctx)`) to suspend evaluation mid-flight.
    /// The returned [`EvalError::Yield`] propagates through dispatch and
    /// surfaces as a [`PlatformError::Yield`] that the host can catch,
    /// serialize, and later resume (see ADR 0040).
    pub fn yield_now(self) -> EvalError {
        EvalError::Yield { context: self }
    }

    /// Serialize the context for cross-machine resumability.
    ///
    /// The `evaluator`, `modules`, and `expr_evaluator` handles are
    /// runtime-only and skipped (see `#[serde(skip)]` on the struct). The
    /// serialized form captures `scopes`, `limits`, `call_count`, and
    /// `handlers` — all the state needed to resume.
    pub fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    /// Deserialize a context for resumption.
    ///
    /// The runtime handles (`evaluator`, `modules`, `expr_evaluator`) are
    /// `None` after deserialization and must be re-attached by the host
    /// before evaluation can resume.
    pub fn from_json(json: &serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(json.clone())
    }

    // -- effect handlers -----------------------------------------------------

    /// Push a handler frame (a list of [`HandlerClause`]s) onto the handler
    /// stack.
    ///
    /// Called by the `handle` eval rule before evaluating the subject
    /// expression. The clauses are active for the duration of the subject
    /// evaluation and removed by [`pop_handlers`].
    ///
    /// [`pop_handlers`]: EvalContext::pop_handlers
    pub fn push_handlers(&mut self, handlers: Vec<HandlerClause>) {
        self.handlers.push(handlers);
    }

    /// Pop the topmost handler frame.
    ///
    /// Called by the `handle` eval rule after the subject expression has been
    /// evaluated. Returns the popped clauses, or `None` if the handler stack
    /// is empty.
    pub fn pop_handlers(&mut self) -> Option<Vec<HandlerClause>> {
        self.handlers.pop()
    }

    /// Find the handler clause matching `operation`, searching from the
    /// outermost (highest-precedence) frame inward.
    ///
    /// Within each frame, the **last** matching clause wins (mirroring the
    /// original interpreter's `.rev()` search). Outer frames take precedence
    /// over inner frames. Returns `None` if no clause matches.
    pub fn find_handler(&self, operation: &str) -> Option<&HandlerClause> {
        for frame in &self.handlers {
            if let Some(clause) = frame.iter().rev().find(|h| h.operation == operation) {
                return Some(clause);
            }
        }
        None
    }

    /// Returns `true` if any handler clause matches `operation`.
    pub fn has_handler(&self, operation: &str) -> bool {
        self.find_handler(operation).is_some()
    }

    /// Returns the number of handler frames on the stack.
    pub fn handler_depth(&self) -> usize {
        self.handlers.len()
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
            .field("handler_depth", &self.handlers.len())
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
            value_callable: self.value_callable.clone(),
            handlers: self.handlers.clone(),
        }
    }
}

impl PartialEq for EvalContext {
    /// Compares only the serializable state (`scopes`, `limits`,
    /// `call_count`, `handlers`). The runtime handles (`evaluator`, `modules`,
    /// `expr_evaluator`) are `Rc<dyn Trait>` pointers that are not
    /// meaningfully comparable and are excluded — consistent with their
    /// `#[serde(skip)]` annotation.
    fn eq(&self, other: &Self) -> bool {
        self.scopes == other.scopes
            && self.limits == other.limits
            && self.call_count == other.call_count
            && self.handlers == other.handlers
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

    /// Create a recursion callback that can be wired into an [`EvalContext`]
    /// via [`EvalContext::set_evaluator`].
    ///
    /// This enables callers (e.g. the CLI's module evaluator) to create a
    /// persistent `EvalContext` that retains bindings across multiple
    /// `ctx.eval(&ast)` calls, rather than getting a fresh context per
    /// dispatch as [`BasicEvaluator::run`] provides.
    pub fn make_callback(&self) -> Rc<dyn EvaluatorCallback> {
        Rc::new(BasicEvaluatorCallback {
            rules: self.rules.clone(),
            exec_controls: self.exec_controls.clone(),
        })
    }

    /// Resume evaluation from a yielded context.
    ///
    /// Re-attaches the evaluator (and, if not already present, the module
    /// lookup) handle to the restored [`EvalContext`], then dispatches `ast`
    /// with it. This is the counterpart to [`EvalError::Yield`] / [`EvalContext::yield_now`]:
    /// the host serializes the yielded context, later deserializes it, and
    /// calls `resume` to continue evaluation (see ADR 0040).
    ///
    /// If the resumed evaluation yields again, the returned
    /// [`PlatformError::Yield`] carries the new context for another
    /// resume cycle.
    pub fn resume(
        &self,
        ast: &AstNode,
        mut context: EvalContext,
        ctx: &mut PluginContext,
    ) -> Result<Value, PlatformError> {
        // Build the recursion handle (clones Arc pointers — cheap).
        let callback = Rc::new(BasicEvaluatorCallback {
            rules: self.rules.clone(),
            exec_controls: self.exec_controls.clone(),
        });
        context.set_evaluator(callback.clone());

        // Re-attach the module lookup if the restored context doesn't have one
        // (handles are skipped during serialization).
        if !context.has_modules() {
            if let Some(modules) = ctx.plugins.module_lookup() {
                context.set_modules(modules);
            }
        }

        let mut req = EvalRequest {
            ast: ast.clone(),
            ctx: context,
            options: ResolvedOptions::new(),
            result: None,
        };

        callback.dispatch(&mut req)?;

        req.result.ok_or(PlatformError::NotImplemented)
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
        ctx: &mut PluginContext,
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
        let mut eval_ctx = EvalContext::new();
        eval_ctx.set_evaluator(callback.clone());

        // Wire up the module lookup so eval rules can resolve
        // module-qualified function calls (e.g. `hello.greet("world")`).
        if let Some(modules) = ctx.plugins.module_lookup() {
            eval_ctx.set_modules(modules);
        }

        let mut req = EvalRequest {
            ast: AstNode {
                kind: kind.to_string(),
                span: None,
                payload: request.payload.clone(),
            },
            ctx: eval_ctx,
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

    #[test]
    fn yield_error_converts_to_platform_yield() {
        let ctx = EvalContext::new();
        let err = ctx.yield_now();
        let pe: PlatformError = err.into();
        assert!(matches!(pe, PlatformError::Yield { .. }));
    }

    // -----------------------------------------------------------------------
    // Yield / resumability tests (ADR 0040)
    // -----------------------------------------------------------------------

    #[test]
    fn eval_context_serialization_round_trip() {
        let mut ctx = EvalContext::new();
        ctx.define("x", serde_json::json!(42));
        ctx.push_scope();
        ctx.define("y", serde_json::json!("hello"));
        ctx.set_limits(EvalLimits {
            max_depth: Some(10),
            max_calls: Some(100),
        });
        // Bump the call counter a few times.
        ctx.bump_call_count();
        ctx.bump_call_count();
        ctx.bump_call_count();

        let json = ctx.to_json().unwrap();
        let restored = EvalContext::from_json(&json).unwrap();

        // Serializable state is preserved.
        assert_eq!(restored, ctx, "scopes/limits/call_count must match");
        assert_eq!(restored.call_count(), 3);
        assert_eq!(restored.depth(), 2);
        assert_eq!(restored.lookup("x"), Some(&serde_json::json!(42)));
        assert_eq!(restored.lookup("y"), Some(&serde_json::json!("hello")));
        assert_eq!(restored.limits().max_depth, Some(10));
        assert_eq!(restored.limits().max_calls, Some(100));

        // Runtime handles are NOT serialized — they are None after restore.
        assert!(!restored.has_evaluator());
        assert!(!restored.has_modules());
    }

    /// A rule that yields on the first call (capturing the context with a
    /// `done` flag set) and returns `42` on resume when `done` is present.
    struct YieldOnce;

    impl Behavior<Eval> for YieldOnce {
        fn kind(&self) -> &str {
            "yield_once"
        }
        fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
            if op.ctx.lookup("done").is_some() {
                op.result = Some(serde_json::json!(42));
            } else {
                // Mark done, then yield — the context is the continuation.
                let mut ctx = std::mem::take(&mut op.ctx);
                ctx.define("done", serde_json::json!(true));
                return Err(ctx.yield_now().into());
            }
            Ok(())
        }
    }
    impl EvalBehavior for YieldOnce {}

    #[test]
    fn yield_mid_evaluation_then_resume() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(YieldOnce) as Arc<dyn EvalBehavior>)
            .unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };

        // First run: the rule yields.
        let err = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"kind": "yield_once"}),
                },
                &mut pctx,
            )
            .unwrap_err();

        let yielded_context = match err {
            PlatformError::Yield { context } => context,
            other => panic!("expected PlatformError::Yield, got {other:?}"),
        };
        // The yielding rule set "done" before yielding.
        assert_eq!(
            yielded_context.lookup("done"),
            Some(&serde_json::json!(true))
        );

        // Serialize → deserialize (simulating cross-machine resumability).
        let json = yielded_context.to_json().unwrap();
        let restored = EvalContext::from_json(&json).unwrap();
        assert_eq!(restored, yielded_context);
        // Handles are gone after deserialization.
        assert!(!restored.has_evaluator());

        // Resume: the rule sees "done" and returns 42.
        let ast = AstNode {
            kind: "yield_once".to_string(),
            span: None,
            payload: serde_json::json!({"kind": "yield_once"}),
        };
        let (mut plugins2, mut kits2) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx2 = PluginContext {
            plugins: &mut plugins2,
            kits: &mut kits2,
        };
        let result = evaluator.resume(&ast, restored, &mut pctx2).unwrap();
        assert_eq!(result, serde_json::json!(42));
    }

    /// A parent rule that evaluates a child node via `ctx.eval`.
    struct ParentRule;
    impl Behavior<Eval> for ParentRule {
        fn kind(&self) -> &str {
            "parent"
        }
        fn apply(&self, op: &mut EvalRequest) -> Result<(), PlatformError> {
            let child = AstNode {
                kind: "yield_once".to_string(),
                span: None,
                payload: serde_json::Value::Null,
            };
            // The child yields — the error must propagate up through us.
            let result = op.ctx.eval(&child)?;
            op.result = Some(result);
            Ok(())
        }
    }
    impl EvalBehavior for ParentRule {}

    #[test]
    fn yield_propagates_through_nested_eval() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(YieldOnce) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator
            .accept(Arc::new(ParentRule) as Arc<dyn EvalBehavior>)
            .unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };

        // The parent rule evaluates a yielding child. The yield must bubble
        // all the way up to `run` as a PlatformError::Yield.
        let err = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: serde_json::json!({"kind": "parent"}),
                },
                &mut pctx,
            )
            .unwrap_err();

        let yielded_context = match err {
            PlatformError::Yield { context } => context,
            other => panic!("expected PlatformError::Yield, got {other:?}"),
        };
        // The child set "done" before yielding — the captured context
        // reflects the state at the yield point.
        assert_eq!(
            yielded_context.lookup("done"),
            Some(&serde_json::json!(true))
        );

        // Resume from the parent: the child now sees "done" and returns 42,
        // which the parent passes through as its result.
        let json = yielded_context.to_json().unwrap();
        let restored = EvalContext::from_json(&json).unwrap();

        let parent_ast = AstNode {
            kind: "parent".to_string(),
            span: None,
            payload: serde_json::json!({"kind": "parent"}),
        };
        let (mut plugins2, mut kits2) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx2 = PluginContext {
            plugins: &mut plugins2,
            kits: &mut kits2,
        };
        let result = evaluator.resume(&parent_ast, restored, &mut pctx2).unwrap();
        assert_eq!(result, serde_json::json!(42));
    }

    // -----------------------------------------------------------------------
    // Async evaluation tests (ADR 0040 — async evaluation)
    // -----------------------------------------------------------------------

    /// `eval_async` dispatches through the same sync `Behavior<Eval>` rules
    /// and wraps the result in a `Future`. Sync rules work unchanged.
    #[test]
    fn eval_async_wraps_sync_rule_result() {
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

        let callback = Rc::new(BasicEvaluatorCallback {
            rules: evaluator.rules.clone(),
            exec_controls: evaluator.exec_controls.clone(),
        });
        let mut ctx = EvalContext::new();
        ctx.set_evaluator(callback);

        let ast = AstNode {
            kind: "echo".to_string(),
            span: None,
            payload: serde_json::json!({"v": 7}),
        };

        // Drive the async future on this thread.
        let fut = ctx.eval_async(&ast);
        let result = futures_block_on(fut).unwrap();
        assert_eq!(result, serde_json::json!({"v": 7}));
    }

    /// `call_function_async` awaits an `AsyncFunction` (advertised via
    /// `Function::as_async`).
    #[test]
    fn call_function_async_uses_async_path() {
        use std::future::ready;
        use std::pin::Pin;

        struct AsyncEcho;
        impl Function for AsyncEcho {
            fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
                Err(EvalError::Other("sync call should not be used".into()))
            }
            fn as_async(&self) -> Option<&dyn AsyncFunction> {
                Some(self)
            }
        }
        impl AsyncFunction for AsyncEcho {
            fn call_async<'a>(
                &'a self,
                args: &'a [Value],
                _ctx: &'a mut EvalContext,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<Value, EvalError>> + 'a>>
            {
                let v = args.first().cloned().unwrap_or(Value::Null);
                Box::pin(ready(Ok(v)))
            }
        }

        struct EchoModule;
        impl Module for EchoModule {
            fn name(&self) -> &str {
                "echo"
            }
            fn function(&self, name: &str) -> Option<&dyn Function> {
                match name {
                    "echo" => Some(&AsyncEcho),
                    _ => None,
                }
            }
        }

        let lookup: Rc<dyn ModuleLookup> = Rc::new(TestRegistry(EchoModule));
        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let args = [serde_json::json!("hi")];
        let fut = ctx.call_function_async("echo.echo", &args);
        let result = futures_block_on(fut).unwrap();
        assert_eq!(result, serde_json::json!("hi"));
    }

    /// `call_function_async` falls back to the sync `call` when the function
    /// does not implement `AsyncFunction`.
    #[test]
    fn call_function_async_falls_back_to_sync() {
        struct SyncEcho;
        impl Function for SyncEcho {
            fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
                Ok(args.first().cloned().unwrap_or(Value::Null))
            }
        }

        struct EchoModule;
        impl Module for EchoModule {
            fn name(&self) -> &str {
                "echo"
            }
            fn function(&self, name: &str) -> Option<&dyn Function> {
                match name {
                    "echo" => Some(&SyncEcho),
                    _ => None,
                }
            }
        }

        let lookup: Rc<dyn ModuleLookup> = Rc::new(TestRegistry(EchoModule));
        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let args = [serde_json::json!(42)];
        let fut = ctx.call_function_async("echo.echo", &args);
        let result = futures_block_on(fut).unwrap();
        assert_eq!(result, serde_json::json!(42));
    }

    /// `call_function_async` returns `UnknownFunction` when no registry is set.
    #[test]
    fn call_function_async_without_registry_returns_unknown_function() {
        let mut ctx = EvalContext::new();
        let args = [serde_json::json!(1)];
        let fut = ctx.call_function_async("math.add", &args);
        let err = futures_block_on(fut).unwrap_err();
        assert_eq!(err, EvalError::UnknownFunction("math.add".to_string()));
    }

    /// A minimal single-module [`ModuleLookup`] for tests, so the platform
    /// crate doesn't need to depend on `plugin-module` (which would be
    /// circular).
    struct TestRegistry<M: Module>(M);
    impl<M: Module + 'static> ModuleLookup for TestRegistry<M> {
        fn lookup(&self, qualified_name: &str) -> Option<&dyn Function> {
            if let Some((module_name, func_name)) = qualified_name.split_once('.') {
                if self.0.name() == module_name {
                    return self.0.function(func_name);
                }
            }
            self.0.function(qualified_name)
        }
    }

    /// A minimal single-threaded future executor for tests.
    ///
    /// Polls a future to completion on the current thread using a no-op waker.
    /// This avoids pulling in an executor dependency (`tokio`/`futures`) just
    /// for the platform's unit tests — the futures here are all ready or
    /// trivially pollable.
    fn futures_block_on<F: std::future::Future>(mut fut: F) -> F::Output {
        use std::pin::Pin;
        use std::sync::Arc;
        use std::task::{Context, Poll, Wake, Waker};

        struct NoopWake;
        impl Wake for NoopWake {
            fn wake(self: Arc<Self>) {}
        }

        let waker: Waker = Arc::new(NoopWake).into();
        let mut cx = Context::from_waker(&waker);
        // SAFETY: `fut` is shadowed and not moved after pinning.
        let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
        loop {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(out) => return out,
                Poll::Pending => {
                    // Our test futures never genuinely park; spin once.
                    std::thread::yield_now();
                }
            }
        }
    }
}
