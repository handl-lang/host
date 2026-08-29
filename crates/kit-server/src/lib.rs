//! # handl-kit-server
//!
//! A kit that curates the server-side library plugins.
//!
//! Includes everything from [`handl-kit-config`] plus:
//!
//! - **OS stdlib** (`fs.*`, `env.*`, `path.*`, `time.*`, `cmd.*`, `process.*`, `tmp.*`, `socket.*`, `sig.*`)
//!   from [`handl-lib-os-std`].
//! - **HTTP server** (`http_server.*`) from [`handl-lib-http-server-std`].
//! - **HTTP client** (`http_client.*`) from [`handl-lib-http-client-std`].
//! - **Random** (`rand.*`) from [`handl-lib-rand`].
//!
//! This is the kit you want when handl needs to open ports, read files,
//! spawn processes, or make HTTP requests.

use handl_kit_config::ConfigKit;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use std::sync::atomic::{AtomicBool, Ordering};

/// A kit that curates the server-side library plugins.
///
/// Registered as `kit.server` in the plugin registry. When contributed, it
/// extends [`ConfigKit`] (pure plugins) with the effectful `plugin-lib-*`
/// library plugins (OS, HTTP, rand, test, otlp).
pub struct ServerKit {
    registered: AtomicBool,
}

impl ServerKit {
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for ServerKit {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ServerKit {
    fn id(&self) -> &str {
        "kit.server"
    }

    fn name(&self) -> &str {
        "kit-server"
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

        ctx.register_plugin(ConfigKit::new())?;

        // Server-side concrete plugins — ConfigKit already registered
        // CanonicalKit (which provides ModuleRuntime) and the pure plugins.
        ctx.register_plugin(handl_plugin_lib_test::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_rand::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_os_std::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_http_server_std::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_http_client_std::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_otlp_std::LibraryPlugin)?;
        ctx.register_plugin(handl_plugin_lib_ws_rfc_native::LibraryPlugin)?;

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
