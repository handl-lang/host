//! # handl-lib-http-server-std
//!
//! Concrete HTTP server implementation backed by `tokio` and `hyper`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The HTTP server interface is declared in
//! `handl-packages/kit-stdlib/http_server/http_server.hnl`, and this crate's `handl`
//! bindings are declared in
//! `handl-packages/kit-stdlib/http_server_std/http_server_std.hnl`. The Rust handls
//! (wrapping [`StdHttpServer`]) are registered in an [`OpRegistry`] via
//! [`http_server_std_op_registry`], which delegates to the shared
//! [`http_server_op_registry`](handl_plugin_lib_http_server::http_server_op_registry)
//! builder. The [`HttpServerStdLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Plugin` trait scaffolding.
//!
//! Provides [`StdHttpServer`] and [`ServerConfig`].

mod http_server;

use std::sync::Arc;

use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_lib_http_server::{http_server_op_registry, HttpServer, HTTP_SERVER_OPS};

pub use handl_plugin_lib_http_server::{
    HttpMethod, HttpServerError, HttpServerRequest, HttpServerResponse,
};
pub use http_server::{ServerConfig, StdHttpServer, StdHttpServerModule, std_http_server_module};

/// The handl source declaring the http-server-std `handl` bindings — the
/// implementation-side declarations for the HTTP server ops. Loaded at
/// compile time from the kit-stdlib data dir.
const HTTP_SERVER_STD_HNL: &str =
    include_str!("../../../handl-packages/kit-stdlib/http_server_std/http_server_std.hnl");

/// Build the [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) for
/// http-server-std, registering the Rust handls (wrapping [`StdHttpServer`])
/// under the op names declared in the http-server interface.
pub fn http_server_std_op_registry() -> handl_plugin_lib_handl::OpRegistry {
    http_server_op_registry(|| {
        Arc::new(StdHttpServer::new()) as Arc<dyn HttpServer>
    })
}

// ---------------------------------------------------------------------------
// HttpServerStdLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `http-server-std` implementation
/// via the generic [`HandlLibraryPlugin`] with an
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
pub struct HttpServerStdLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl HttpServerStdLibraryPlugin {
    /// Create a new `HttpServerStdLibraryPlugin` backed by [`StdHttpServer`].
    pub fn new() -> Self {
        let registry = Arc::new(http_server_std_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "http_server",
                "lib.http-server-std",
                "lib-http-server-std",
                HTTP_SERVER_STD_HNL,
                &[],
            )
            .with_ops(HTTP_SERVER_OPS, registry),
        }
    }
}

impl Default for HttpServerStdLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for HttpServerStdLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for http-server-std with the op registry
/// (backed by [`StdHttpServer`]).
pub fn http_server_std_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(http_server_std_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "http_server",
        "lib.http-server-std",
        "lib-http-server-std",
        HTTP_SERVER_STD_HNL,
        &[],
    )
    .with_ops(HTTP_SERVER_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`HttpServerStdLibraryPlugin`].
pub use HttpServerStdLibraryPlugin as LibraryPlugin;
