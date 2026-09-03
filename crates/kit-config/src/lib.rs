//! # handl-kit-config
//!
//! A kit that curates the pure (side-effect-free) library plugins.
//!
//! Provides only pure (side-effect-free) library modules. No I/O, no side
//! effects, no `import()`.
//!
//! A host that provides only pure functions is automatically sandboxed —
//! this is a policy decision, not a language feature.
//!
//! ## What's included
//!
//! - **`math.*`**, **`str.*`**, **`array.*`**, **`set.*`**, **`bool.*`**,
//!   **`cmp.*`**, **`obj.*`**, **`fn.*`**, **`type.*`** — from [`handl-lib-utils`]
//! - **`crypto.*`** — SHA-256, MD5, Base64, hex — from [`handl-lib-crypto`]
//! - **`datetime.*`** — parsing, formatting, arithmetic — from [`handl-lib-datetime`]
//! - **`intl.*`** — locale-aware formatting (ICU4X) — from [`handl-lib-intl`]
//! - **`kv.*`** — in-memory key-value store — from [`handl-lib-kv`] + [`handl-lib-kv-hashmap`]
//!
//! ## What's excluded
//!
//! - [`handl-lib-rand`] — non-deterministic RNG
//! - [`handl-lib-os`] / [`handl-lib-os-std`] — filesystem, env,
//!   process spawning, system clock
//! - [`handl-lib-http-client`] / [`handl-lib-http-client-std`] — HTTP client

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use std::sync::atomic::{AtomicBool, Ordering};

/// A kit that curates the pure (side-effect-free) library plugins.
///
/// Registered as `kit.config` in the plugin registry. When contributed, it
/// registers every pure `plugin-lib-*` library plugin.
pub struct ConfigKit {
    registered: AtomicBool,
}

impl ConfigKit {
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for ConfigKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ConfigKit {
    fn id(&self) -> &str {
        "kit.config"
    }

    fn name(&self) -> &str {
        "kit-config"
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

        // Register ModuleRuntime so library plugins can register their modules.
        ctx.register_plugin(handl_plugin_module::ModuleRuntime::default())?;

        // Library plugins — these register modules (math, str, crypto, etc.)
        // into the ModuleRuntime. The language operations (lexer, parser,
        // evaluator) are now provided by the data-driven
        // `handl-kit-canonical::Kit` backed by `handl-runtime`.
        ctx.register_plugin(handl_plugin_lib_utils::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_cmp::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_crypto::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_datetime::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_intl::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_kv_hashmap::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_collections::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_unicode::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_stats::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_ws_rfc_memory::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_ws_whatwg_native::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_ip::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_compress_gzip::LibraryPlugin::new())?;
        ctx.register_plugin(handl_plugin_lib_archive_tar::LibraryPlugin::new())?;

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
