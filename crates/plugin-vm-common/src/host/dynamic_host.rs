//! A host assembled dynamically from plugin contributions.
//!
//! [`DynamicHost`] combines the built-in functions provided by [`EvalHost`]
//! with library modules contributed by `plugin-lib-*` plugins (collected in a
//! [`ModuleHost`]). Resource limits (scope depth, call count) can be applied
//! via [`DynamicHost::with_max_depth`] / [`DynamicHost::with_max_calls`] and
//! are enforced on the [`CanonicalContext`] returned by
//! [`DynamicHost::new_context`].
//!
//! This replaces the hardcoded `SandboxHost` / `ServerHost` / `CliHost`
//! structs: a kit curates its plugins, their `contribute` methods register
//! modules into the [`ModuleHost`], and [`DynamicHost`] wraps the result.

use handl_platform::{
    ContributionContext, KitRegistry, PlatformError, Plugin, PluginRegistry,
};
use handl_plugin_vm_host_eval::EvalHost;
use handl_plugin_vm_values_json::{
    CanonicalContext, Function, GenericFunction, Host, LazyFunction, NonFunctionValue,
};

use crate::host::ModuleHost;

/// A host assembled dynamically from plugin contributions.
///
/// Combines built-in functions ([`EvalHost`]) with library modules
/// contributed by `plugin-lib-*` plugins ([`ModuleHost`]). Resource limits
/// can be applied via the builder methods.
#[derive(Clone)]
pub struct DynamicHost {
    default: EvalHost,
    modules: ModuleHost,
    max_depth: Option<usize>,
    max_calls: Option<usize>,
}

impl DynamicHost {
    /// Assembles a host from a list of plugins.
    ///
    /// A [`ModuleHost`] is registered first so that `plugin-lib-*` plugins
    /// can contribute their modules to it via
    /// `ctx.contribute::<ModuleHost>(ModuleRule::new(...))`. Each plugin's
    /// [`Plugin::contribute`] method is then called, allowing modules to be
    /// registered into the [`ModuleHost`]. The assembled [`ModuleHost`] is
    /// extracted and combined with a default [`EvalHost`].
    pub fn from_plugins<I, P>(plugins: I) -> Result<Self, PlatformError>
    where
        I: IntoIterator<Item = P>,
        P: Plugin + 'static,
    {
        let mut registry = PluginRegistry::new();
        registry.register(ModuleHost::new());
        for plugin in plugins {
            registry.register(plugin);
        }
        Self::from_registry(registry)
    }

    /// Assembles a host from a pre-built [`PluginRegistry`].
    ///
    /// The registry must already contain a [`ModuleHost`] (registered first).
    /// Each plugin's [`Plugin::contribute`] method is called, allowing modules
    /// to be registered into the [`ModuleHost`]. The assembled [`ModuleHost`]
    /// is extracted and combined with a default [`EvalHost`].
    pub fn from_registry(mut registry: PluginRegistry) -> Result<Self, PlatformError> {
        let mut kits = KitRegistry::new();
        let len = registry.len();
        for index in 0..len {
            let Some(plugin) = registry.take_at(index) else {
                continue;
            };
            {
                let mut ctx = ContributionContext::new(&mut registry, &mut kits);
                plugin.contribute(&mut ctx)?;
            }
            registry.replace_at(index, plugin);
        }

        let modules = registry
            .get_plugin_mut::<ModuleHost>()
            .ok_or_else(|| PlatformError::Plugin("ModuleHost not found in registry".into()))?
            .clone();

        Ok(Self {
            default: EvalHost::default(),
            modules,
            max_depth: None,
            max_calls: None,
        })
    }

    /// Creates a `DynamicHost` from an already-assembled [`ModuleHost`].
    ///
    /// This is the simpler entry point for callers (e.g. a kit's `contribute`
    /// method) that have already assembled their modules: it just wraps the
    /// [`ModuleHost`] with a default [`EvalHost`] and no resource limits.
    pub fn new(modules: ModuleHost) -> Self {
        Self {
            default: EvalHost::default(),
            modules,
            max_depth: None,
            max_calls: None,
        }
    }

    /// Sets the maximum scope depth (function call nesting).
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Sets the maximum number of function calls during a single evaluation.
    pub fn with_max_calls(mut self, calls: usize) -> Self {
        self.max_calls = Some(calls);
        self
    }

    /// Returns the inner [`ModuleHost`].
    pub fn modules(&self) -> &ModuleHost {
        &self.modules
    }

    /// Returns the configured maximum scope depth, if any.
    pub fn max_depth(&self) -> Option<usize> {
        self.max_depth
    }

    /// Returns the configured maximum call count, if any.
    pub fn max_calls(&self) -> Option<usize> {
        self.max_calls
    }

    /// Creates a [`CanonicalContext`] with the configured resource limits
    /// applied.
    ///
    /// The limits (`max_depth`, `max_calls`) are enforced by the VM during
    /// evaluation. The expression [`Eval`] callback is intentionally left
    /// unset here — it lives in the VM backend crate, which cannot be a
    /// dependency of this crate (it depends on `handl-plugin-vm-common`).
    /// Callers that have access to the backend (e.g. a kit) should install it
    /// via `ctx.set_eval(Box::new(CanonicalEval::new(host, max_depth,
    /// max_calls)))` if nested expression evaluation is required.
    ///
    /// [`Eval`]: handl_plugin_vm::Eval
    pub fn new_context(&self) -> CanonicalContext {
        let mut ctx = CanonicalContext::new();
        if let Some(max) = self.max_depth {
            ctx.set_max_depth(max);
        }
        if let Some(max) = self.max_calls {
            ctx.set_max_calls(max);
        }
        ctx
    }
}

impl Default for DynamicHost {
    fn default() -> Self {
        Self::new(ModuleHost::new())
    }
}

impl Host<NonFunctionValue> for DynamicHost {
    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        self.default
            .function(name)
            .or_else(|| self.modules.function(name))
    }

    fn lazy_function(&self, name: &str) -> Option<&dyn LazyFunction<NonFunctionValue>> {
        self.default
            .lazy_function(name)
            .or_else(|| self.modules.lazy_function(name))
    }

    fn generic_function(&self, name: &str) -> Option<&dyn GenericFunction<NonFunctionValue>> {
        self.default
            .generic_function(name)
            .or_else(|| self.modules.generic_function(name))
    }

    fn is_reserved(&self, name: &str) -> bool {
        self.default.is_reserved(name) || self.modules.is_reserved(name)
    }
}

#[cfg(test)]
mod tests {
    use super::DynamicHost;
    use crate::host::{ModuleHost, ModuleRule};
    use crate::module::module_stub::StubModule;
    use handl_platform::{
        CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
        ContributionContext, PlatformError, Plugin, PluginContext,
    };
    use handl_plugin_vm_values_json::Host;

    /// A test plugin that contributes [`StubModule`] into the [`ModuleHost`].
    struct StubPlugin;

    impl Plugin for StubPlugin {
        fn id(&self) -> &str {
            "test.stub-plugin"
        }
        fn name(&self) -> &str {
            "stub-plugin"
        }
        fn capabilities(&self) -> &[CapabilityId] {
            &[]
        }
        fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
            None
        }
        fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
            ctx.contribute::<ModuleHost>(ModuleRule::new(StubModule))?;
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

    #[test]
    fn from_plugins_contributes_modules() {
        // `StubPlugin::contribute` registers `StubModule` into the
        // `ModuleHost` via the contribution pipeline.
        let host = DynamicHost::from_plugins([StubPlugin]).expect("assembly succeeds");
        assert!(host.function("stub.echo").is_some());
        assert_eq!(host.modules().module_count(), 1);
    }

    #[test]
    fn new_wraps_preassembled_modules() {
        let host = DynamicHost::new(ModuleHost::new().with(StubModule));
        assert!(host.function("stub.echo").is_some());
    }

    #[test]
    fn empty_host_has_no_functions() {
        let host = DynamicHost::default();
        assert!(host.function("stub.echo").is_none());
        assert_eq!(host.modules().module_count(), 0);
    }

    #[test]
    fn builder_sets_limits() {
        let host = DynamicHost::default().with_max_depth(50).with_max_calls(1_000);
        let ctx = host.new_context();
        assert_eq!(ctx.max_depth(), Some(50));
        assert_eq!(ctx.max_calls(), Some(1_000));
        // Accessors reflect the configured limits.
        assert_eq!(host.max_depth(), Some(50));
        assert_eq!(host.max_calls(), Some(1_000));
    }
}
