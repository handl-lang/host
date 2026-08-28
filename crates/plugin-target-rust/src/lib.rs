//! Runtime plugin backed by the Rust VM interpreter.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use handl_expr::{Pattern, Stmt, StmtKind};
use handl_platform::{
    CapabilityId, CapabilityRequest, CapabilityResponse, PlatformError, Plugin, PluginContext,
    RuntimeBehavior, RuntimeHost,
};
use handl_runtime::{EvalOutput, RuntimePlugin};
use handl_vm::{Context, Error, Function, Host, VirtualMachine};
use handl_vm_backend_rust_interpreter::{
    CanonicalContext, CanonicalEval, CanonicalVm, NonFunctionValue,
};
use handl_vm_common::ModuleHost;

/// A `RuntimePlugin` adapter around the canonical Rust VM.
pub struct RustVm {
    capabilities: Vec<CapabilityId>,
    modules: ModuleHost,
    rules: Mutex<Vec<Arc<dyn RuntimeBehavior>>>,
}

impl RustVm {
    /// Creates a new `RustVm` plugin with the built-in host and no extra libraries.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("runtime")],
            modules: ModuleHost::new(),
            rules: Mutex::new(Vec::new()),
        }
    }

    /// Creates a new `RustVm` with the given pre-installed library `ModuleHost`.
    pub fn with_modules(modules: ModuleHost) -> Self {
        Self {
            capabilities: vec![CapabilityId::from("runtime")],
            modules,
            rules: Mutex::new(Vec::new()),
        }
    }
}

impl Default for RustVm {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimePlugin for RustVm {
    fn name(&self) -> &str {
        "vm-rust"
    }

    fn eval(&self, stmts: &[Stmt<NonFunctionValue>]) -> Result<EvalOutput, PlatformError> {
        let rules = self.rules.lock().unwrap().clone();
        let host = CompositeHost::new(&rules, self.modules.clone());
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(CanonicalEval::new(host.clone(), None, None)));
        let vm = CanonicalVm::new(host);

        let mut values = Vec::new();
        for stmt in stmts {
            match &stmt.kind {
                StmtKind::Expr(expr) => match vm.eval(expr, &mut ctx) {
                    Ok(value) => values.push(value),
                    Err(e) => return Err(PlatformError::Plugin(e.to_string())),
                },
                StmtKind::Let { pattern, value } | StmtKind::Const { pattern, value } => {
                    match vm.eval(value, &mut ctx) {
                        Ok(value) => {
                            if let Err(e) = define_simple(&mut ctx, pattern, value) {
                                return Err(PlatformError::Plugin(e.to_string()));
                            }
                        }
                        Err(e) => return Err(PlatformError::Plugin(e.to_string())),
                    }
                }
                _ => {}
            }
        }

        Ok(EvalOutput { values })
    }
}

impl Plugin for RustVm {
    fn name(&self) -> &str {
        "vm-rust"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<serde_json::Value> {
        Some(serde_json::json!({}))
    }

    fn contribute(&self, ctx: &mut handl_platform::ContributionContext) -> Result<(), PlatformError> {
        // Snapshot any runtime rules that have already been contributed to the
        // platform `RuntimeHost`.
        if let Some(host) = ctx.get_plugin_mut::<RuntimeHost>() {
            *self.rules.lock().unwrap() = host.rules().to_vec();
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

fn define_simple(
    ctx: &mut dyn Context<NonFunctionValue>,
    pattern: &Pattern,
    value: NonFunctionValue,
) -> Result<(), Error<NonFunctionValue>> {
    match pattern {
        Pattern::Identifier(name) => {
            ctx.define(name, value);
            Ok(())
        }
        _ => Err(Error::PrimitiveError(
            "unsupported binding pattern".to_string(),
        )),
    }
}

/// A host that combines contributed built-in functions with a `ModuleHost` of loaded libraries.
#[derive(Clone)]
struct CompositeHost {
    functions: HashMap<String, Arc<dyn Function<NonFunctionValue>>>,
    modules: ModuleHost,
}

impl CompositeHost {
    fn new(rules: &[Arc<dyn RuntimeBehavior>], modules: ModuleHost) -> Self {
        let mut functions = HashMap::new();
        for rule in rules {
            for &name in rule.provides() {
                let f: Arc<dyn Function<NonFunctionValue>> = Arc::new(RuntimeFunction {
                    rule: rule.clone(),
                    name: name.to_string(),
                });
                functions.insert(name.to_string(), f);
            }
        }
        Self { functions, modules }
    }
}

impl Host<NonFunctionValue> for CompositeHost {
    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        self.functions
            .get(name)
            .map(|f| f.as_ref())
            .or_else(|| self.modules.function(name))
    }

    fn lazy_function(&self, name: &str) -> Option<&dyn handl_vm::LazyFunction<NonFunctionValue>> {
        self.modules.lazy_function(name)
    }

    fn generic_function(
        &self,
        name: &str,
    ) -> Option<&dyn handl_vm::GenericFunction<NonFunctionValue>> {
        self.modules.generic_function(name)
    }

    fn async_function(&self, name: &str) -> Option<&dyn handl_vm::AsyncFunction<NonFunctionValue>> {
        self.modules.async_function(name)
    }

    fn operation(&self, name: &str) -> Option<&dyn handl_vm::Operation<NonFunctionValue>> {
        self.modules.operation(name)
    }

    fn is_reserved(&self, name: &str) -> bool {
        self.functions.contains_key(name) || self.modules.is_reserved(name)
    }
}

struct RuntimeFunction {
    rule: Arc<dyn RuntimeBehavior>,
    name: String,
}

impl Function<NonFunctionValue> for RuntimeFunction {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        self.rule.call(&self.name, args, ctx)
    }
}
