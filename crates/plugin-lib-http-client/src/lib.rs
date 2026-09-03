//! # handl-lib-http-client
//!
//! Abstract HTTP client service trait for the handl runtime.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The HTTP client interface is declared in
//! `kit-stdlib/data/http_client/http_client.hnl` via `op` declarations,
//! and the Rust handls (handlers) that delegate to an [`HttpClient`] are
//! registered in an [`OpRegistry`] via [`http_client_op_registry`]. The
//! [`HttpClientLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`]
//! with the op registry (backed by [`StubHttpClient`] by default),
//! eliminating the per-library `Plugin` trait scaffolding.
//!
//! ## Architecture
//!
//! ```text
//! handl-plugin-vm-common          handl-lib-http-client            handl-lib-http-client-std
//! ─────────────            ─────────────────────         ───────────────────────────
//! Module trait             HttpClient trait              StdHttpClient (ureq)
//! ModuleRuntime               HttpMethod/HttpRequest/HttpResponse
//!                          HttpError
//!                          HttpClientLibraryPlugin      HttpClientStdLibraryPlugin
//! ```
//!
//! - **This crate** — abstract trait + module adapter + test stub.
//!   No I/O, no platform dependencies.
//! - **`handl-lib-http-client-std`** — concrete implementation using `ureq`.

mod error;
mod http_client;

use std::sync::Arc;

use handl_plugin_lib_handl::HandlLibraryPlugin;

pub use error::HttpError;

pub use http_client::{
    register_http_client_ops, HttpClient, HttpClientModule, HttpMethod, HttpRequest,
    HttpResponse, HTTP_CLIENT_OPS,
};

// Re-export stub for convenience.
pub use http_client::http_client_stub::StubHttpClient;

/// The handl source declaring the http-client `op` interface — the single
/// source of truth for the HTTP client API surface. Loaded at compile time
/// from the kit-stdlib data dir.
const HTTP_CLIENT_HNL: &str =
    include_str!("../../../kit-stdlib/data/http_client/http_client.hnl");

/// Build an [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) for the
/// HTTP client ops, registering Rust handls that delegate to the given
/// [`HttpClient`] under their op names.
pub fn http_client_op_registry(client: impl HttpClient + 'static) -> handl_plugin_lib_handl::OpRegistry {
    let mut registry = handl_plugin_lib_handl::OpRegistry::new();
    register_http_client_ops(&mut registry, client);
    registry
}

// ---------------------------------------------------------------------------
// HttpClientLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `http-client` interface via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The HTTP client interface is declared in `http_client.hnl` via
/// `op`/`handl` declarations; the Rust handls (delegating to
/// [`StubHttpClient`] by default) are registered in the op registry.
///
/// Concrete backends such as `handl-lib-http-client-std` provide their own
/// plugin ([`HttpClientStdLibraryPlugin`]) with a real [`HttpClient`]
/// implementation.
pub struct HttpClientLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl HttpClientLibraryPlugin {
    /// Create a new `HttpClientLibraryPlugin` backed by [`StubHttpClient`].
    pub fn new() -> Self {
        let registry = Arc::new(http_client_op_registry(StubHttpClient::default()));
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "http",
                "lib.http-client",
                "lib-http-client",
                HTTP_CLIENT_HNL,
                &[],
            )
            .with_ops(HTTP_CLIENT_OPS, registry),
        }
    }
}

impl Default for HttpClientLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for HttpClientLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for http-client with the op registry
/// (backed by [`StubHttpClient`]).
pub fn http_client_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(http_client_op_registry(StubHttpClient::default()));
    HandlLibraryPlugin::new_with_caps(
        "http",
        "lib.http-client",
        "lib-http-client",
        HTTP_CLIENT_HNL,
        &[],
    )
    .with_ops(HTTP_CLIENT_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`HttpClientLibraryPlugin`].
pub use HttpClientLibraryPlugin as LibraryPlugin;
