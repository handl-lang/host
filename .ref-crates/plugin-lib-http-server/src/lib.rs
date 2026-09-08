//! # handl-lib-http-server
//!
//! Abstract HTTP server service trait and module adapter for the handl runtime.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The HTTP server interface is declared in
//! `handl-packages/http_server/http_server.hnl` via `op` declarations,
//! and the Rust handls (handlers) that delegate to an [`HttpServer`] are
//! registered in an [`OpRegistry`] via [`http_server_op_registry`]. The
//! [`HttpServerLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`]
//! with the op registry (backed by [`StubHttpServer`] by default),
//! eliminating the per-library `Plugin` trait scaffolding.
//!
//! ## Architecture
//!
//! ```text
//! handl-plugin-vm-common          handl-lib-http-server            handl-lib-http-server-std
//! ─────────────            ─────────────────────         ───────────────────────────
//! Module trait             HttpServer trait              StdHttpServer
//! ModuleRuntime               HttpServerRequest/Response
//!                          HttpServerError
//!                          HttpServerLibraryPlugin      HttpServerStdLibraryPlugin
//! ```
//!
//! - **This crate** — abstract trait + module adapter + test stub.
//! - **`handl-lib-http-server-std`** — concrete implementation.

mod error;
pub mod function_value;
mod http_method;
mod http_server;
mod request;
mod response;

use std::sync::Arc;

use handl_plugin_lib_handl::HandlLibraryPlugin;

pub use error::HttpServerError;
pub use http_method::HttpMethod;
pub use http_server::{
    HTTP_SERVER_OPS, Handler, HttpServer, HttpServerModule, StubHttpServer,
    register_http_server_ops,
};
pub use request::HttpServerRequest;
pub use response::HttpServerResponse;

/// The handl source declaring the http-server `op` interface — the single
/// source of truth for the HTTP server API surface. Loaded at compile time
/// from the kit-stdlib data dir.
const HTTP_SERVER_HNL: &str = include_str!("../../../handl-packages/http_server/http_server.hnl");

/// Build an [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) for the
/// HTTP server ops, registering Rust handls that delegate to
/// [`HttpServer`] instances created by the given factory.
///
/// The factory `F` must be `Fn() -> Arc<dyn HttpServer> + Send + Sync + 'static`.
pub fn http_server_op_registry<F>(factory: F) -> handl_plugin_lib_handl::OpRegistry
where
    F: Fn() -> Arc<dyn HttpServer> + Send + Sync + 'static,
{
    let mut registry = handl_plugin_lib_handl::OpRegistry::new();
    register_http_server_ops(&mut registry, factory);
    registry
}

// ---------------------------------------------------------------------------
// HttpServerLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `http-server` interface via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The HTTP server interface is declared in `http_server.hnl` via
/// `op`/`handl` declarations; the Rust handls (delegating to
/// [`StubHttpServer`] by default) are registered in the op registry.
///
/// Concrete backends such as `handl-lib-http-server-std` provide their own
/// plugin ([`HttpServerStdLibraryPlugin`]) with a real [`HttpServer`]
/// implementation.
pub struct HttpServerLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl HttpServerLibraryPlugin {
    /// Create a new `HttpServerLibraryPlugin` backed by [`StubHttpServer`].
    pub fn new() -> Self {
        let registry = Arc::new(http_server_op_registry(|| {
            Arc::new(StubHttpServer::new()) as Arc<dyn HttpServer>
        }));
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "http_server",
                "lib.http-server",
                "lib-http-server",
                HTTP_SERVER_HNL,
                &[],
            )
            .with_ops(HTTP_SERVER_OPS, registry),
        }
    }
}

impl Default for HttpServerLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for HttpServerLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for http-server with the op registry
/// (backed by [`StubHttpServer`]).
pub fn http_server_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(http_server_op_registry(|| {
        Arc::new(StubHttpServer::new()) as Arc<dyn HttpServer>
    }));
    HandlLibraryPlugin::new_with_caps(
        "http_server",
        "lib.http-server",
        "lib-http-server",
        HTTP_SERVER_HNL,
        &[],
    )
    .with_ops(HTTP_SERVER_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`HttpServerLibraryPlugin`].
pub use HttpServerLibraryPlugin as LibraryPlugin;
