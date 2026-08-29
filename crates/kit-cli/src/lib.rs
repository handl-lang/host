//! # handl-kit-cli
//!
//! A kit for building CLI tools with handl.
//!
//! Extends `kit-canonical` with OS and utility library plugins so handl
//! expressions can access filesystem, environment, path operations, string
//! utilities, JSON/TOML/YAML parsing, and more.
//!
//! ## What's included
//!
//! - Everything from `kit-canonical` (lexer, parser, formatter, linter,
//!   typechecker, evaluator, compiler, runtime)
//! - **OS stdlib** (`fs.*`, `env.*`, `path.*`, `time.*`, `cmd.*`, `process.*`,
//!   `tmp.*`, `socket.*`, `sig.*`) from `handl-lib-os-std`
//! - **Utils** (`math.*`, `str.*`, `bool.*`, `cmp.*`, `fn.*`, `type.*`,
//!   `option.*`, `result.*`, `json.*`, `toml.*`, `yaml.*`, etc.) from
//!   `handl-lib-utils`
//! - **Collections** (`array.*`, `set.*`, `map.*`) from `handl-lib-collections`
//!
//! ## Usage
//!
//! ### As a platform kit (plugin composition)
//!
//! ```ignore
//! use handl_platform::DefaultHost;
//! use handl_kit_cli::CliKit;
//!
//! let mut host = DefaultHost::new();
//! host.register_plugin(CliKit::new());
//! host.load().unwrap();
//! ```
//!
//! ### As a VM host (expression evaluation)
//!
//! ```ignore
//! use handl_kit_cli::{CliHost, CliLimits};
//! use handl_plugin_vm_backend_rust_interpreter::Vm;
//!
//! let host = CliHost::new(CliLimits::default());
//! let ctx = host.new_context();
//! let vm = Vm::new(host);
//! ```

use handl_kit_canonical::CanonicalKit;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_collections::CollectionsModule;
use handl_plugin_lib_dedent::DedentModule;
use handl_plugin_lib_json::JsonModule;
use handl_plugin_lib_os::{
    CmdModule, EnvModule, FsModule, PathModule, ProcessModule, SigModule, SocketModule, TimeModule,
    TmpModule,
};
use handl_plugin_lib_os_std::{
    StdCmd, StdEnv, StdFs, StdPath, StdProcess, StdSig, StdSocket, StdTime, StdTmp,
};
use handl_plugin_lib_toml::TomlModule;
use handl_plugin_lib_utils::{
    Base64Module, BoolModule, CmpModule, EncodingModule, FnModule, HexModule, MathModule,
    MoodModule, OptionModule, ParseModule, ResultModule, SerializeModule, StrModule, TypeModule,
};
use handl_plugin_lib_yaml::YamlModule;
use handl_plugin_vm_backend_rust_interpreter::CanonicalEval;
use handl_plugin_vm_common::ModuleHost;
use handl_plugin_vm_host_eval::EvalHost;
use handl_plugin_vm_values_json::{
    CanonicalContext, Function, GenericFunction, Host, LazyFunction, NonFunctionValue,
};
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Platform plugin — composes canonical kit + library plugins
// ---------------------------------------------------------------------------

/// A kit that composes the canonical kit plus OS and utility library plugins.
///
/// Registered as `kit.cli` in the plugin registry. When contributed, it
/// registers `CanonicalKit` (language operations) and all CLI-relevant
/// library plugins.
pub struct CliKit {
    registered: AtomicBool,
}

impl CliKit {
    /// Create a new `CliKit`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for CliKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CliKit {
    fn id(&self) -> &str {
        "kit.cli"
    }

    fn name(&self) -> &str {
        "kit-cli"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if self.registered.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.registered.store(true, Ordering::SeqCst);

        ctx.register_plugin(CanonicalKit::new())?;

        ctx.register_plugin(handl_plugin_lib_os::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_utils::LibraryPlugin)?;

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
// VM host — provides function lookup for expression evaluation
// ---------------------------------------------------------------------------

/// Resource limits for CLI evaluation.
#[derive(Clone, Debug)]
pub struct CliLimits {
    /// Maximum scope depth (function call nesting).
    pub max_depth: Option<usize>,
    /// Maximum number of function calls during a single evaluation.
    pub max_calls: Option<usize>,
}

impl Default for CliLimits {
    fn default() -> Self {
        CliLimits {
            max_depth: Some(256),
            max_calls: Some(100_000),
        }
    }
}

/// A VM host with OS, utils, and collections modules for CLI tool development.
///
/// Function lookup checks the built-in [`EvalHost`] first, then the
/// [`ModuleHost`] (OS, utils, collections, etc.).
#[derive(Clone)]
pub struct CliHost {
    default: EvalHost,
    modules: ModuleHost,
    limits: CliLimits,
}

impl CliHost {
    /// Creates a new CLI host with the given limits and all modules registered.
    pub fn new(limits: CliLimits) -> Self {
        let modules = ModuleHost::new()
            // Utils
            .with(MathModule)
            .with(StrModule)
            .with(BoolModule)
            .with(CmpModule)
            .with(FnModule)
            .with(TypeModule)
            .with(Base64Module)
            .with(HexModule)
            .with(JsonModule)
            .with(TomlModule)
            .with(YamlModule)
            .with(EncodingModule::new())
            .with(ParseModule::new())
            .with(SerializeModule::new())
            .with(OptionModule)
            .with(ResultModule)
            .with(MoodModule)
            // Dedent
            .with(DedentModule)
            // Collections
            .with(CollectionsModule)
            // OS stdlib
            .with(FsModule::new(StdFs))
            .with(EnvModule::new(StdEnv))
            .with(PathModule::new(StdPath))
            .with(TimeModule::new(StdTime))
            .with(CmdModule::new(StdCmd))
            .with(ProcessModule::new(StdProcess::default()))
            .with(TmpModule::new(StdTmp))
            .with(SocketModule::new(StdSocket::default()))
            .with(SigModule::new(StdSig));

        CliHost {
            default: EvalHost::default(),
            modules,
            limits,
        }
    }

    /// Creates a [`Context`] with the CLI limits applied.
    pub fn new_context(&self) -> CanonicalContext {
        let mut ctx = CanonicalContext::new();
        if let Some(max) = self.limits.max_depth {
            ctx.set_max_depth(max);
        }
        if let Some(max) = self.limits.max_calls {
            ctx.set_max_calls(max);
        }
        ctx.set_eval(Box::new(CanonicalEval::new(
            self.clone(),
            self.limits.max_depth,
            self.limits.max_calls,
        )));
        ctx
    }

    /// Returns the configured limits.
    pub fn limits(&self) -> &CliLimits {
        &self.limits
    }

    /// Returns the inner [`ModuleHost`], allowing callers to register
    /// additional modules on top of the CLI defaults.
    pub fn modules(&self) -> &ModuleHost {
        &self.modules
    }

    /// Consumes the host and returns the inner [`ModuleHost`].
    pub fn into_modules(self) -> ModuleHost {
        self.modules
    }
}

impl Default for CliHost {
    fn default() -> Self {
        Self::new(CliLimits::default())
    }
}

impl Host<NonFunctionValue> for CliHost {
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
    use super::{CliHost, CliLimits};
    use handl_plugin_vm_values_json::Host;

    #[test]
    fn os_functions_are_available() {
        let host = CliHost::new(CliLimits::default());
        assert!(host.function("fs.read").is_some());
        assert!(host.function("env.get").is_some());
        assert!(host.function("path.join").is_some());
        assert!(host.function("cmd.run").is_some());
    }

    #[test]
    fn utils_functions_are_available() {
        let host = CliHost::new(CliLimits::default());
        assert!(host.function("str.upper").is_some());
        assert!(host.function("math.abs").is_some());
        assert!(host.function("cmp.eq").is_some());
        assert!(host.function("fn.pipe").is_some());
    }

    #[test]
    fn fn_module_array_ops_are_available() {
        let host = CliHost::new(CliLimits::default());
        assert!(host.function("fn.map").is_some());
        assert!(host.function("fn.filter").is_some());
        assert!(host.function("fn.reduce").is_some());
    }

    #[test]
    fn json_toml_yaml_are_available() {
        let host = CliHost::new(CliLimits::default());
        assert!(host.function("json").is_some());
        assert!(host.function("toml").is_some());
        assert!(host.function("yaml").is_some());
        assert!(host.function("serialize").is_some());
        assert!(host.function("deserialize").is_some());
    }

    #[test]
    fn dedent_is_available() {
        let host = CliHost::new(CliLimits::default());
        assert!(host.function("dedent.dedent").is_some());
    }
}
