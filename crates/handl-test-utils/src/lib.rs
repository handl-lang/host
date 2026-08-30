//! # handl-test-utils
//!
//! Test mocks, stubs, and helpers for the eval / module / exec-control
//! infrastructure introduced by ADR 0040.
//!
//! This is **not** a plugin — it is a test-utilities crate that other crates
//! depend on (as a dev-dependency, or a regular dependency gated behind a
//! `cfg(test)` / feature) when they need to exercise the platform's
//! `Eval` operation, `Module`/`Function` registration, or `ExecControl` hooks
//! without standing up a full host.
//!
//! ## What's here
//!
//! - [`MockModule`] / [`MockFunction`] / [`CaptureFn`] / [`NoopModule`] —
//!   configurable `Module`/`Function` implementations for testing function
//!   registration and lookup. Ported from `plugin-vm-common`'s `StubModule`
//!   and `CaptureFn`, adapted to the non-generic `Module`/`Function` traits
//!   from `handl-platform`.
//! - [`test_context`] / [`test_context_with_scope`] /
//!   [`test_context_with_module`] — helpers that build an [`EvalContext`]
//!   pre-seeded with variables and/or a module registry.
//! - [`MockEvalRule`] — a `Behavior<Eval>` rule that returns a configurable
//!   value, for testing evaluator dispatch.
//! - [`MockExecControl`] — an `ExecControl` that counts `before`/`after` hook
//!   invocations, for testing exec-control wiring.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use handl_platform::{
    AstNode, Behavior, ControlFlow, Eval, EvalBehavior, EvalContext, EvalError, EvalRequest,
    ExecControl, Function, Module, ModuleLookup, PlatformError,
};
use handl_plugin_module::ModuleRegistry;

/// Conformance test harness for `plugin-engine-*` engines (ADR-0044).
pub mod conformance;

/// Closure type backing [`MockFunction`] and [`CaptureFnWithHandler`].
type FnHandler = Box<dyn Fn(&[Value], &mut EvalContext) -> Result<Value, EvalError> + Send + Sync>;

// ---------------------------------------------------------------------------
// MockModule / MockFunction / CaptureFn / NoopModule
// ---------------------------------------------------------------------------

/// A configurable [`Module`] for testing.
///
/// Built via a builder API:
///
/// ```ignore
/// use handl_test_utils::{MockModule, MockFunction};
/// use handl_platform::{EvalContext, EvalError, Function};
/// use serde_json::{Value, json};
///
/// let add = MockFunction::new("add", |args, _ctx| {
///     let a = args[0].as_i64().unwrap();
///     let b = args[1].as_i64().unwrap();
///     Ok(json!(a + b))
/// });
/// let module = MockModule::new("math").with_function("add", add);
/// assert_eq!(module.name(), "math");
/// assert!(module.function("add").is_some());
/// assert!(module.function("sub").is_none());
/// ```
///
/// Ported from `plugin-vm-common`'s `StubModule`, adapted to the
/// non-generic `Module` trait from `handl-platform`.
pub struct MockModule {
    name: String,
    functions: HashMap<String, Arc<dyn Function>>,
}

impl MockModule {
    /// Create a new empty module with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        MockModule {
            name: name.into(),
            functions: HashMap::new(),
        }
    }

    /// Register a function (builder style).
    pub fn with_function(
        mut self,
        name: impl Into<String>,
        function: impl Function + 'static,
    ) -> Self {
        self.functions.insert(name.into(), Arc::new(function));
        self
    }

    /// Register an already-`Arc`ed function (builder style).
    pub fn with_function_arc(
        mut self,
        name: impl Into<String>,
        function: Arc<dyn Function>,
    ) -> Self {
        self.functions.insert(name.into(), function);
        self
    }

    /// Number of registered functions.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
}

impl Module for MockModule {
    fn name(&self) -> &str {
        &self.name
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.functions.get(name).map(|f| f.as_ref())
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.functions
            .iter()
            .map(|(name, f)| (name.as_str(), f.as_ref()))
            .collect()
    }
}

/// A [`Function`] backed by a closure, for testing.
///
/// The closure receives the argument slice and a mutable [`EvalContext`],
/// matching the `Function::call` signature. This lets tests define inline
/// behavior without writing a one-off struct.
///
/// ```ignore
/// use handl_test_utils::MockFunction;
/// use serde_json::{Value, json};
///
/// let echo = MockFunction::new("echo", |args, _ctx| Ok(args[0].clone()));
/// ```
pub struct MockFunction {
    name: String,
    handler: FnHandler,
}

impl MockFunction {
    /// Create a new mock function with the given name and handler closure.
    pub fn new(
        name: impl Into<String>,
        handler: impl Fn(&[Value], &mut EvalContext) -> Result<Value, EvalError> + Send + Sync + 'static,
    ) -> Self {
        MockFunction {
            name: name.into(),
            handler: Box::new(handler),
        }
    }

    /// The function's name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Function for MockFunction {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        (self.handler)(args, ctx)
    }
}

/// A [`Function`] that records every call made to it and returns a
/// configurable result.
///
/// Ported from `plugin-vm-common`'s `CaptureFn`, adapted to the non-generic
/// `Function` trait. Unlike the original (which wrapped an inner closure),
/// this version returns a fixed [`Value`] configured at construction time.
/// Use [`CaptureFn::with_handler`] if you need custom return logic.
///
/// ```ignore
/// use handl_test_utils::CaptureFn;
/// use serde_json::{Value, json};
///
/// let capture = CaptureFn::new(json!(42));
/// let mut ctx = handl_platform::EvalContext::new();
/// let result = capture.call(&[json!(1)], &mut ctx).unwrap();
/// assert_eq!(result, json!(42));
/// assert_eq!(capture.calls(), vec![vec![json!(1)]]);
/// ```
pub struct CaptureFn {
    calls: Mutex<Vec<Vec<Value>>>,
    return_value: Value,
}

impl CaptureFn {
    /// Create a `CaptureFn` that always returns `return_value`.
    pub fn new(return_value: Value) -> Self {
        CaptureFn {
            calls: Mutex::new(Vec::new()),
            return_value,
        }
    }

    /// Returns the recorded argument lists, one `Vec<Value>` per call.
    pub fn calls(&self) -> Vec<Vec<Value>> {
        self.calls.lock().unwrap().clone()
    }

    /// Number of calls recorded so far.
    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

impl Function for CaptureFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        self.calls.lock().unwrap().push(args.to_vec());
        Ok(self.return_value.clone())
    }
}

/// A [`CaptureFn`] that delegates to a custom handler closure while still
/// recording calls.
pub struct CaptureFnWithHandler {
    calls: Mutex<Vec<Vec<Value>>>,
    handler: FnHandler,
}

impl CaptureFnWithHandler {
    /// Create a capturing function backed by `handler`.
    pub fn with_handler(
        handler: impl Fn(&[Value], &mut EvalContext) -> Result<Value, EvalError> + Send + Sync + 'static,
    ) -> Self {
        CaptureFnWithHandler {
            calls: Mutex::new(Vec::new()),
            handler: Box::new(handler),
        }
    }

    /// Returns the recorded argument lists, one `Vec<Value>` per call.
    pub fn calls(&self) -> Vec<Vec<Value>> {
        self.calls.lock().unwrap().clone()
    }

    /// Number of calls recorded so far.
    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

impl Function for CaptureFnWithHandler {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        self.calls.lock().unwrap().push(args.to_vec());
        (self.handler)(args, ctx)
    }
}

/// A [`Module`] with no functions, for tests that need a module but don't
/// care about function lookup.
///
/// ```ignore
/// use handl_test_utils::NoopModule;
/// use handl_platform::Module;
///
/// let module = NoopModule::new("empty");
/// assert_eq!(module.name(), "empty");
/// assert!(module.function("anything").is_none());
/// ```
pub struct NoopModule {
    name: String,
}

impl NoopModule {
    /// Create a new empty module with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        NoopModule { name: name.into() }
    }
}

impl Module for NoopModule {
    fn name(&self) -> &str {
        &self.name
    }

    fn function(&self, _name: &str) -> Option<&dyn Function> {
        None
    }
}

// ---------------------------------------------------------------------------
// EvalContext helpers
// ---------------------------------------------------------------------------

/// Create a fresh [`EvalContext`] with a single empty global scope, no
/// evaluator handle, and no module registry.
pub fn test_context() -> EvalContext {
    EvalContext::new()
}

/// Create an [`EvalContext`] seeded with the given variables in the global
/// scope.
///
/// ```ignore
/// use handl_test_utils::test_context_with_scope;
/// use serde_json::{Value, json};
///
/// let ctx = test_context_with_scope(&[("x", json!(42)), ("y", json!("hi"))]);
/// assert_eq!(ctx.lookup("x"), Some(&json!(42)));
/// assert_eq!(ctx.lookup("y"), Some(&json!("hi")));
/// ```
pub fn test_context_with_scope(vars: &[(&str, Value)]) -> EvalContext {
    let mut ctx = EvalContext::new();
    for (name, value) in vars {
        ctx.define(*name, value.clone());
    }
    ctx
}

/// Create an [`EvalContext`] wired with a [`ModuleRegistry`] containing the
/// given module, so `ctx.call_function` resolves functions from it.
pub fn test_context_with_module(module: impl Module + 'static) -> EvalContext {
    let registry = ModuleRegistry::new().with(module);
    let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);
    let mut ctx = EvalContext::new();
    ctx.set_modules(lookup);
    ctx
}

// ---------------------------------------------------------------------------
// MockEvalRule
// ---------------------------------------------------------------------------

/// A `Behavior<Eval>` rule for testing that returns a configurable value.
///
/// The rule matches a specific `AstNode.kind` (set at construction) and, on
/// `apply`, writes the configured `result` into `req.result`. This lets tests
/// exercise `BasicEvaluator` dispatch without writing a one-off rule struct.
///
/// ```ignore
/// use handl_test_utils::MockEvalRule;
/// use handl_platform::{BasicEvaluator, CapabilityRequest, PluginContext,
///     PluginRegistry, KitRegistry, EvalBehavior};
/// use serde_json::json;
/// use std::sync::Arc;
///
/// let mut evaluator = BasicEvaluator::new();
/// evaluator.accept(Arc::new(MockEvalRule::new("echo", json!(42))) as Arc<dyn EvalBehavior>).unwrap();
/// ```
pub struct MockEvalRule {
    kind: String,
    result: Value,
}

impl MockEvalRule {
    /// Create a new mock eval rule matching `kind` and returning `result`.
    pub fn new(kind: impl Into<String>, result: Value) -> Self {
        MockEvalRule {
            kind: kind.into(),
            result,
        }
    }

    /// The AST kind this rule matches.
    pub fn kind_str(&self) -> &str {
        &self.kind
    }

    /// The value this rule returns on dispatch.
    pub fn result_value(&self) -> &Value {
        &self.result
    }
}

impl Behavior<Eval> for MockEvalRule {
    fn kind(&self) -> &str {
        &self.kind
    }

    fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
        req.result = Some(self.result.clone());
        Ok(())
    }
}

impl EvalBehavior for MockEvalRule {}

// ---------------------------------------------------------------------------
// MockExecControl
// ---------------------------------------------------------------------------

/// An [`ExecControl`] for testing that counts `before_eval` / `after_eval`
/// invocations.
///
/// Each hook always returns [`ControlFlow::Continue`] (i.e. it never blocks
/// or short-circuits dispatch). Use the [`before_count`] /
/// [`after_count`] accessors to assert how many times each hook fired.
///
/// ```ignore
/// use handl_test_utils::MockExecControl;
/// use std::sync::Arc;
///
/// let control = Arc::new(MockExecControl::new());
/// assert_eq!(control.before_count(), 0);
/// ```
///
/// [`before_count`]: MockExecControl::before_count
/// [`after_count`]: MockExecControl::after_count
pub struct MockExecControl {
    before_count: AtomicUsize,
    after_count: AtomicUsize,
}

impl MockExecControl {
    /// Create a new `MockExecControl` with both counters at zero.
    pub fn new() -> Self {
        MockExecControl {
            before_count: AtomicUsize::new(0),
            after_count: AtomicUsize::new(0),
        }
    }

    /// Number of times `before_eval` has been called.
    pub fn before_count(&self) -> usize {
        self.before_count.load(Ordering::SeqCst)
    }

    /// Number of times `after_eval` has been called.
    pub fn after_count(&self) -> usize {
        self.after_count.load(Ordering::SeqCst)
    }
}

impl Default for MockExecControl {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecControl for MockExecControl {
    fn before_eval(
        &self,
        _ast: &mut AstNode,
        _ctx: &mut EvalContext,
    ) -> Result<ControlFlow, PlatformError> {
        self.before_count.fetch_add(1, Ordering::SeqCst);
        Ok(ControlFlow::Continue)
    }

    fn after_eval(
        &self,
        _ast: &AstNode,
        _result: &Value,
        _ctx: &mut EvalContext,
    ) -> Result<(), PlatformError> {
        self.after_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        BasicEvaluator, CapabilityRequest, KitRegistry, Plugin, PluginContext, PluginRegistry,
        RuleAcceptor,
    };
    use serde_json::{Value, json};
    use std::rc::Rc;

    // --- MockModule ---

    #[test]
    fn mock_module_registers_and_looks_up_functions() {
        let add = MockFunction::new("add", |args, _ctx| {
            let a = args[0].as_i64().unwrap();
            let b = args[1].as_i64().unwrap();
            Ok(json!(a + b))
        });
        let module = MockModule::new("math").with_function("add", add);

        assert_eq!(module.name(), "math");
        assert!(module.function("add").is_some());
        assert!(module.function("sub").is_none());
        assert_eq!(module.function_count(), 1);
    }

    #[test]
    fn mock_module_functions_enumeration() {
        let add = MockFunction::new("add", |_, _| Ok(json!(0)));
        let mul = MockFunction::new("mul", |_, _| Ok(json!(0)));
        let module = MockModule::new("math")
            .with_function("add", add)
            .with_function("mul", mul);

        let fns = module.functions();
        assert_eq!(fns.len(), 2);
        let names: Vec<&str> = fns.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"add"));
        assert!(names.contains(&"mul"));
    }

    #[test]
    fn mock_module_function_is_callable_through_module() {
        let echo = MockFunction::new("echo", |args, _ctx| Ok(args[0].clone()));
        let module = MockModule::new("echo_mod").with_function("echo", echo);

        let func = module.function("echo").expect("echo should be registered");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!("hello")], &mut ctx).unwrap();
        assert_eq!(result, json!("hello"));
    }

    // --- CaptureFn ---

    #[test]
    fn capture_fn_records_calls_and_returns_configured_value() {
        let capture = CaptureFn::new(json!(42));
        let mut ctx = EvalContext::new();

        let r1 = capture.call(&[json!(1), json!(2)], &mut ctx).unwrap();
        let r2 = capture.call(&[json!(3)], &mut ctx).unwrap();

        assert_eq!(r1, json!(42));
        assert_eq!(r2, json!(42));
        assert_eq!(capture.call_count(), 2);
        assert_eq!(
            capture.calls(),
            vec![vec![json!(1), json!(2)], vec![json!(3)]]
        );
    }

    #[test]
    fn capture_fn_with_handler_records_and_delegates() {
        let capture = CaptureFnWithHandler::with_handler(|args, _ctx| {
            Ok(json!(args.iter().filter_map(Value::as_i64).sum::<i64>()))
        });
        let mut ctx = EvalContext::new();

        let result = capture.call(&[json!(1), json!(2), json!(3)], &mut ctx).unwrap();
        assert_eq!(result, json!(6));
        assert_eq!(capture.call_count(), 1);
        assert_eq!(capture.calls(), vec![vec![json!(1), json!(2), json!(3)]]);
    }

    // --- NoopModule ---

    #[test]
    fn noop_module_resolves_nothing() {
        let module = NoopModule::new("empty");
        assert_eq!(module.name(), "empty");
        assert!(module.function("anything").is_none());
        assert!(module.functions().is_empty());
    }

    // --- test_context helpers ---

    #[test]
    fn test_context_with_scope_seeds_variables() {
        let ctx = test_context_with_scope(&[("x", json!(42)), ("y", json!("hi"))]);
        assert_eq!(ctx.lookup("x"), Some(&json!(42)));
        assert_eq!(ctx.lookup("y"), Some(&json!("hi")));
        assert_eq!(ctx.lookup("missing"), None);
    }

    #[test]
    fn test_context_is_fresh() {
        let ctx = test_context();
        assert_eq!(ctx.depth(), 1);
        assert!(!ctx.has_evaluator());
        assert!(!ctx.has_modules());
    }

    #[test]
    fn test_context_with_module_resolves_functions() {
        let add = MockFunction::new("add", |args, _ctx| {
            Ok(json!(args[0].as_i64().unwrap() + args[1].as_i64().unwrap()))
        });
        let module = MockModule::new("math").with_function("add", add);

        let mut ctx = test_context_with_module(module);
        assert!(ctx.has_modules());

        let result = ctx.call_function("math.add", &[json!(2), json!(3)]).unwrap();
        assert_eq!(result, json!(5));
    }

    // --- MockEvalRule ---

    #[test]
    fn mock_eval_rule_returns_configured_value_when_dispatched() {
        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(MockEvalRule::new("echo", json!(42))) as Arc<dyn EvalBehavior>)
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
                    payload: json!({"kind": "echo"}),
                },
                &mut pctx,
            )
            .unwrap();
        assert_eq!(resp.0, json!(42));
    }

    #[test]
    fn mock_eval_rule_kind_and_result_accessors() {
        let rule = MockEvalRule::new("greet", json!("hello"));
        assert_eq!(rule.kind_str(), "greet");
        assert_eq!(rule.result_value(), &json!("hello"));
    }

    // --- MockExecControl ---

    #[test]
    fn mock_exec_control_counts_hooks_correctly() {
        let control = Arc::new(MockExecControl::new());
        assert_eq!(control.before_count(), 0);
        assert_eq!(control.after_count(), 0);

        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(MockEvalRule::new("echo", json!(1))) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator.add_exec_control(control.clone());

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let _ = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: json!({"kind": "echo"}),
                },
                &mut pctx,
            )
            .unwrap();

        assert_eq!(control.before_count(), 1);
        assert_eq!(control.after_count(), 1);
    }

    #[test]
    fn mock_exec_control_default_is_zeroed() {
        let control = MockExecControl::default();
        assert_eq!(control.before_count(), 0);
        assert_eq!(control.after_count(), 0);
    }

    // --- integration: MockModule + CaptureFn via ModuleRegistry ---

    #[test]
    fn capture_fn_inside_mock_module_records_via_registry() {
        let capture = Arc::new(CaptureFn::new(json!(true)));
        let module = MockModule::new("flag").with_function_arc("get", capture.clone());

        let registry = ModuleRegistry::new().with(module);
        let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);

        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let result = ctx.call_function("flag.get", &[json!("arg")]).unwrap();
        assert_eq!(result, json!(true));
        assert_eq!(capture.call_count(), 1);
        assert_eq!(capture.calls(), vec![vec![json!("arg")]]);
    }

    #[test]
    fn mock_eval_rule_and_exec_control_compose_through_run() {
        // MockEvalRule returns a value; MockExecControl counts hooks; both
        // wired through the public BasicEvaluator::run path.
        let control = Arc::new(MockExecControl::new());

        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(MockEvalRule::new("echo", json!(7))) as Arc<dyn EvalBehavior>)
            .unwrap();
        evaluator.add_exec_control(control.clone());

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = evaluator
            .run(
                &CapabilityRequest {
                    capability: "eval".into(),
                    payload: json!({"kind": "echo"}),
                },
                &mut pctx,
            )
            .unwrap();

        assert_eq!(resp.0, json!(7));
        assert_eq!(control.before_count(), 1);
        assert_eq!(control.after_count(), 1);
    }
}
