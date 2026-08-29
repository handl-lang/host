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

use handl_kit_canonical::CanonicalKit;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
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

        // Concrete library plugins — these register modules (math, str, fs,
        // json, etc.) into the ModuleRuntime that CanonicalKit provides.
        ctx.register_plugin(handl_plugin_lib_os_std::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_utils::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_collections::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_json::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_toml::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_yaml::LibraryPlugin)?;

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
