//! # handl-plugin-module
//!
//! Function registration mechanism for the platform, per ADR 0040.
//!
//! This crate provides:
//!
//! - **[`ModuleRegistry`]** — a struct holding registered [`Module`]s and
//!   implementing the platform's [`ModuleLookup`] trait. It resolves
//!   qualified names like `math.add` (split on the first `.`) or flat names
//!   like `add` (searched across all modules).
//! - **[`ModuleRuntime`]** — a [`Plugin`] + [`RuleAcceptor`] that accepts
//!   [`ModuleRule`] contributions (each wrapping one [`Module`]) and exposes
//!   the accumulated registry. Ported from `plugin-vm-common::host::host`.
//! - **[`ModuleRule`]** — a [`Contribution`] wrapping a single [`Module`].
//!
//! The [`Module`] and [`Function`] traits themselves live in `handl-platform`
//! (they are core abstractions, like `Behavior` and `Operation`), and are
//! re-exported here for convenience.
//!
//! `EvalContext` holds an `Option<Rc<dyn ModuleLookup>>`; the host wires a
//! `ModuleRegistry` (via `Rc::new`) into the context before evaluation so
//! eval rules can call `ctx.call_function("math.add", &[1, 2])`.

use std::sync::Arc;

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, Contribution,
    PlatformError, Plugin, PluginContext, RuleAcceptor,
};

// Re-export the core traits so consumers of `plugin-module` don't need a
// direct dependency on `handl-platform` just for `Module`/`Function`.
pub use handl_platform::{Function, Module, ModuleLookup};

// ---------------------------------------------------------------------------
// ModuleRegistry
// ---------------------------------------------------------------------------

/// A registry of [`Module`]s providing qualified function lookup.
///
/// `ModuleRegistry` implements [`ModuleLookup`]. It splits a qualified name
/// on the first `.`: the left segment is the module name, the right segment
/// is the function name within that module. For example, `math.add` resolves
/// to module `"math"`, function `"add"`.
///
/// If the name contains no `.`, the registry searches all modules in
/// registration order and returns the first match. This allows flat names
/// (e.g. `add`) to coexist with qualified names (e.g. `math.add`).
///
/// # Architecture
///
/// ```text
///        ModuleRegistry
///            │
///            │ lookup("math.add")
///            ▼
///      split on "."
///       ┌────┴────┐
///       ▼         ▼
///   "math"     "add"
///       │         │
///       ▼         ▼
///    Module    Function
/// ```
///
/// The eval layer sees only the [`ModuleLookup`] interface — it does not know
/// about modules.
#[derive(Clone, Default)]
pub struct ModuleRegistry {
    modules: Vec<Arc<dyn Module + Send + Sync>>,
}

impl ModuleRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        ModuleRegistry {
            modules: Vec::new(),
        }
    }

    /// Register a module (builder style).
    pub fn with(mut self, module: impl Module + Send + Sync + 'static) -> Self {
        self.modules.push(Arc::new(module));
        self
    }

    /// Register a module (mutating).
    pub fn register(&mut self, module: impl Module + Send + Sync + 'static) {
        self.modules.push(Arc::new(module));
    }

    /// Register an already-`Arc`ed module (mutating).
    pub fn register_arc(&mut self, module: Arc<dyn Module + Send + Sync>) {
        self.modules.push(module);
    }

    /// Returns the number of registered modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Returns an iterator over the registered module names.
    pub fn module_names(&self) -> impl Iterator<Item = &str> {
        self.modules.iter().map(|m| m.name())
    }

    /// Look up a module by name.
    pub fn module(&self, name: &str) -> Option<&Arc<dyn Module + Send + Sync>> {
        self.modules.iter().find(|m| m.name() == name)
    }
}

impl ModuleLookup for ModuleRegistry {
    fn lookup(&self, qualified_name: &str) -> Option<&dyn Function> {
        // First, try each module with the full qualified name — some modules
        // (e.g. `collections`) use dotted function names like `vec.new`.
        for m in &self.modules {
            if let Some(f) = m.function(qualified_name) {
                return Some(f);
            }
        }
        // Fall back to splitting on the first dot: `module_name.func_name`.
        if let Some((module_name, func_name)) = qualified_name.split_once('.') {
            self.modules
                .iter()
                .find(|m| m.name() == module_name)
                .and_then(|m| m.function(func_name))
        } else {
            None
        }
    }
}

impl std::fmt::Debug for ModuleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleRegistry")
            .field("module_count", &self.modules.len())
            .field(
                "module_names",
                &self.modules.iter().map(|m| m.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ModuleRule (Contribution)
// ---------------------------------------------------------------------------

/// A [`Contribution`] that registers a single [`Module`] with a
/// [`ModuleRuntime`] [`RuleAcceptor`].
///
/// This wraps a module in an `Arc` so it can be passed through the platform
/// contribution pipeline via
/// `ctx.contribute::<ModuleRuntime>(ModuleRule::new(MyModule))`.
pub struct ModuleRule(pub Arc<dyn Module + Send + Sync>);

impl ModuleRule {
    /// Wrap a module for contribution to a [`ModuleRuntime`].
    pub fn new(module: impl Module + Send + Sync + 'static) -> Self {
        Self(Arc::new(module))
    }

    /// Wrap an already-`Arc`ed module for contribution.
    pub fn from_arc(module: Arc<dyn Module + Send + Sync>) -> Self {
        Self(module)
    }
}

impl Contribution for ModuleRule {
    fn id(&self) -> &str {
        self.0.name()
    }
}

// ---------------------------------------------------------------------------
// ModuleRuntime (Plugin + RuleAcceptor)
// ---------------------------------------------------------------------------

/// A [`Plugin`] + [`RuleAcceptor`] that accepts [`ModuleRule`] contributions
/// and provides function lookup via the accumulated [`ModuleRegistry`].
///
/// Ported from `plugin-vm-common::host::host::ModuleRuntime`. The old
/// `ModuleRuntime` implemented the VM's `CallableLookup<V>` trait; this new
/// version implements the platform's non-generic [`ModuleLookup`] trait
/// instead. Hosts register a `ModuleRuntime` plugin, library plugins
/// contribute `ModuleRule`s into it, and the host wires the runtime's
/// registry into `EvalContext` via `set_modules`.
///
/// # Example
///
/// ```ignore
/// use std::rc::Rc;
/// use handl_platform::{DefaultHost, ModuleLookup};
/// use handl_plugin_module::{ModuleRuntime, ModuleRule, ModuleRegistry};
///
/// let mut host = DefaultHost::default();
/// host.register_plugin(ModuleRuntime::new());
/// // ... library plugins contribute ModuleRule(MyModule) ...
/// host.load();
/// let runtime = host.plugins_mut().get_plugin_mut::<ModuleRuntime>().unwrap();
/// let registry: Rc<dyn ModuleLookup> = Rc::new(runtime.registry().clone());
/// ```
#[derive(Clone, Default)]
pub struct ModuleRuntime {
    registry: ModuleRegistry,
}

impl ModuleRuntime {
    /// Create a new empty `ModuleRuntime`.
    pub fn new() -> Self {
        ModuleRuntime {
            registry: ModuleRegistry::new(),
        }
    }

    /// Register a module directly (builder style).
    pub fn with(mut self, module: impl Module + Send + Sync + 'static) -> Self {
        self.registry.register(module);
        self
    }

    /// Returns the number of registered modules.
    pub fn module_count(&self) -> usize {
        self.registry.module_count()
    }

    /// Borrow the underlying [`ModuleRegistry`].
    pub fn registry(&self) -> &ModuleRegistry {
        &self.registry
    }
}

impl Plugin for ModuleRuntime {
    fn id(&self) -> &str {
        "runtime.module-host"
    }

    fn name(&self) -> &str {
        "module-host"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }

    fn module_lookup(&self) -> Option<std::rc::Rc<dyn handl_platform::ModuleLookup>> {
        Some(std::rc::Rc::new(self.clone()))
    }
}

impl RuleAcceptor for ModuleRuntime {
    type Rule = ModuleRule;

    fn accept(&mut self, rule: ModuleRule) -> Result<(), PlatformError> {
        self.registry.register_arc(rule.0);
        Ok(())
    }
}

impl ModuleLookup for ModuleRuntime {
    fn lookup(&self, qualified_name: &str) -> Option<&dyn Function> {
        self.registry.lookup(qualified_name)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        BasicEvaluator, CapabilityRequest, ContributionContext, DefaultHost, EvalContext,
        EvalError, KitRegistry, PluginContext, PluginRegistry, RuleAcceptor,
    };
    use serde_json::{Value, json};
    use std::rc::Rc;

    // --- mock module + functions ---

    /// `math.add(x, y) -> x + y`
    struct Add;
    impl Function for Add {
        fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            let a = args
                .first()
                .ok_or_else(|| EvalError::TypeMismatch("add: missing arg 0".into()))?;
            let b = args
                .get(1)
                .ok_or_else(|| EvalError::TypeMismatch("add: missing arg 1".into()))?;
            // Preserve integer arithmetic when both args are integers.
            if let (Some(ai), Some(bi)) = (a.as_i64(), b.as_i64()) {
                return Ok(json!(ai + bi));
            }
            let af = a
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("add: arg 0 must be a number".into()))?;
            let bf = b
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("add: arg 1 must be a number".into()))?;
            Ok(json!(af + bf))
        }
    }

    /// `math.mul(x, y) -> x * y`
    struct Mul;
    impl Function for Mul {
        fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            let a = args
                .first()
                .ok_or_else(|| EvalError::TypeMismatch("mul: missing arg 0".into()))?;
            let b = args
                .get(1)
                .ok_or_else(|| EvalError::TypeMismatch("mul: missing arg 1".into()))?;
            if let (Some(ai), Some(bi)) = (a.as_i64(), b.as_i64()) {
                return Ok(json!(ai * bi));
            }
            let af = a
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("mul: arg 0 must be a number".into()))?;
            let bf = b
                .as_f64()
                .ok_or_else(|| EvalError::TypeMismatch("mul: arg 1 must be a number".into()))?;
            Ok(json!(af * bf))
        }
    }

    /// `greet(name) ~> "hello, {name}"`
    struct Greet;
    impl Function for Greet {
        fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            let name = args.first().and_then(Value::as_str).unwrap_or("world");
            Ok(json!(format!("hello, {name}")))
        }
    }

    struct MathModule;
    impl Module for MathModule {
        fn name(&self) -> &str {
            "math"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            match name {
                "add" => Some(&Add),
                "mul" => Some(&Mul),
                _ => None,
            }
        }
        fn functions(&self) -> Vec<(&str, &dyn Function)> {
            vec![
                ("add", &Add as &dyn Function),
                ("mul", &Mul as &dyn Function),
            ]
        }
    }

    struct GreetModule;
    impl Module for GreetModule {
        fn name(&self) -> &str {
            "greet"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            match name {
                "greet" => Some(&Greet),
                _ => None,
            }
        }
    }

    // --- registry tests ---

    #[test]
    fn registry_lookup_qualified_name() {
        let registry = ModuleRegistry::new().with(MathModule).with(GreetModule);
        let func = registry.lookup("math.add");
        assert!(func.is_some());
        assert!(registry.lookup("math.mul").is_some());
        assert!(registry.lookup("greet.greet").is_some());
    }

    #[test]
    fn registry_lookup_unknown_qualified_returns_none() {
        let registry = ModuleRegistry::new().with(MathModule);
        assert!(registry.lookup("math.sub").is_none());
        assert!(registry.lookup("nope.add").is_none());
    }

    #[test]
    fn registry_lookup_flat_name_searches_all_modules() {
        let registry = ModuleRegistry::new().with(MathModule).with(GreetModule);
        // "greet" is a flat name found in GreetModule.
        assert!(registry.lookup("greet").is_some());
        // "add" is a flat name found in MathModule.
        assert!(registry.lookup("add").is_some());
        // Unknown flat name.
        assert!(registry.lookup("missing").is_none());
    }

    #[test]
    fn registry_module_count_and_names() {
        let registry = ModuleRegistry::new().with(MathModule).with(GreetModule);
        assert_eq!(registry.module_count(), 2);
        let names: Vec<&str> = registry.module_names().collect();
        assert_eq!(names, vec!["math", "greet"]);
    }

    #[test]
    fn registry_module_by_name() {
        let registry = ModuleRegistry::new().with(MathModule);
        assert!(registry.module("math").is_some());
        assert!(registry.module("greet").is_none());
    }

    #[test]
    fn registry_functions_enumeration() {
        let module = MathModule;
        let fns = module.functions();
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].0, "add");
        assert_eq!(fns[1].0, "mul");
    }

    // --- call_function via EvalContext ---

    #[test]
    fn eval_context_call_function_works_with_registered_modules() {
        let registry = ModuleRegistry::new().with(MathModule);
        let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);

        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let result = ctx
            .call_function("math.add", &[json!(2), json!(3)])
            .unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn eval_context_call_function_flat_name() {
        let registry = ModuleRegistry::new().with(GreetModule);
        let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);

        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let result = ctx.call_function("greet", &[json!("alice")]).unwrap();
        assert_eq!(result, json!("hello, alice"));
    }

    #[test]
    fn eval_context_call_function_without_registry_returns_unknown_function() {
        let mut ctx = EvalContext::new();
        let err = ctx.call_function("math.add", &[json!(1)]).unwrap_err();
        assert_eq!(err, EvalError::UnknownFunction("math.add".to_string()));
    }

    #[test]
    fn eval_context_call_function_unknown_name_returns_unknown_function() {
        let registry = ModuleRegistry::new().with(MathModule);
        let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);

        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let err = ctx.call_function("math.sub", &[json!(1)]).unwrap_err();
        assert_eq!(err, EvalError::UnknownFunction("math.sub".to_string()));
    }

    #[test]
    fn eval_context_has_modules_reflects_set_modules() {
        let mut ctx = EvalContext::new();
        assert!(!ctx.has_modules());

        let registry = ModuleRegistry::new().with(MathModule);
        ctx.set_modules(Rc::new(registry));
        assert!(ctx.has_modules());
    }

    // --- ModuleRuntime plugin tests ---

    #[test]
    fn module_runtime_accepts_module_rule() {
        let mut runtime = ModuleRuntime::new();
        runtime.accept(ModuleRule::new(MathModule)).unwrap();
        assert_eq!(runtime.module_count(), 1);
        assert!(runtime.lookup("math.add").is_some());
    }

    #[test]
    fn module_runtime_plugin_id_and_name() {
        let runtime = ModuleRuntime::new();
        assert_eq!(runtime.id(), "runtime.module-host");
        assert_eq!(runtime.name(), "module-host");
    }

    #[test]
    fn module_runtime_implements_module_lookup() {
        let mut runtime = ModuleRuntime::new();
        runtime.accept(ModuleRule::new(MathModule)).unwrap();
        runtime.accept(ModuleRule::new(GreetModule)).unwrap();

        let lookup: &dyn ModuleLookup = &runtime;
        assert!(lookup.lookup("math.add").is_some());
        assert!(lookup.lookup("greet.greet").is_some());
        assert!(lookup.lookup("math.sub").is_none());
    }

    #[test]
    fn module_runtime_contributes_through_host() {
        let mut host = DefaultHost::default();
        host.register_plugin(ModuleRuntime::new());

        // Manually contribute a module rule (simulating a library plugin).
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        // Register the runtime in the test plugin registry too.
        plugins.register(ModuleRuntime::new());
        let mut cctx = ContributionContext::new(&mut plugins, &mut kits);
        cctx.contribute::<ModuleRuntime>(ModuleRule::new(MathModule))
            .unwrap();

        let runtime = plugins
            .get_plugin_mut::<ModuleRuntime>()
            .expect("ModuleRuntime should be registered");
        assert_eq!(runtime.module_count(), 1);
        assert!(runtime.lookup("math.add").is_some());
    }

    #[test]
    fn module_rule_id_is_module_name() {
        let rule = ModuleRule::new(MathModule);
        assert_eq!(rule.id(), "math");
    }

    #[test]
    fn module_runtime_run_returns_not_implemented() {
        let runtime = ModuleRuntime::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut pctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = runtime
            .run(
                &CapabilityRequest {
                    capability: "anything".into(),
                    payload: Value::Null,
                },
                &mut pctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::NotImplemented));
    }

    /// Verify `call_function` works end-to-end inside a `BasicEvaluator`
    /// dispatch: a rule that calls `math.add` through the context.
    #[test]
    fn call_function_works_inside_eval_rule() {
        use handl_platform::{Behavior, Eval, EvalBehavior, EvalRequest, PlatformError};
        use std::sync::Arc;

        struct CallAdd;
        impl Behavior<Eval> for CallAdd {
            fn kind(&self) -> &str {
                "call_add"
            }
            fn apply(&self, req: &mut EvalRequest) -> Result<(), PlatformError> {
                let a = req.ast.payload.get("a").cloned().unwrap_or(json!(0));
                let b = req.ast.payload.get("b").cloned().unwrap_or(json!(0));
                let result = req.ctx.call_function("math.add", &[a, b])?;
                req.result = Some(result);
                Ok(())
            }
        }
        impl EvalBehavior for CallAdd {}

        let registry = ModuleRegistry::new().with(MathModule);
        let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);

        let mut evaluator = BasicEvaluator::new();
        evaluator
            .accept(Arc::new(CallAdd) as Arc<dyn handl_platform::EvalBehavior>)
            .unwrap();

        // We can't easily set modules through BasicEvaluator::run yet (that
        // wiring is a later step), so test call_function directly on a
        // context that has the lookup wired.
        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);
        let result = ctx
            .call_function("math.add", &[json!(40), json!(2)])
            .unwrap();
        assert_eq!(result, json!(42));
    }
}
