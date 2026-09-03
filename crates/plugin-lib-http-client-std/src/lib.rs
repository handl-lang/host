//! # handl-lib-http-client-std
//!
//! Concrete HTTP client implementation backed by `ureq`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The HTTP client interface is declared in
//! `kit-stdlib/data/http_client/http_client.hnl`, and this crate's `handl`
//! bindings are declared in
//! `kit-stdlib/data/http_client_std/http_client_std.hnl`. The Rust handls
//! (wrapping [`StdHttpClient`]) are registered in an [`OpRegistry`] via
//! [`http_client_std_op_registry`], which delegates to the shared
//! [`http_client_op_registry`](handl_plugin_lib_http_client::http_client_op_registry)
//! builder. The [`HttpClientStdLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the per-library
//! `Plugin` trait scaffolding.
//!
//! Provides [`StdHttpClient`] using the [`ureq`] crate.

mod http_client;

use std::sync::Arc;

use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_lib_http_client::{http_client_op_registry, HTTP_CLIENT_OPS};

pub use http_client::StdHttpClient;

/// The handl source declaring the http-client-std `handl` bindings — the
/// implementation-side declarations for the HTTP client ops. Loaded at
/// compile time from the kit-stdlib data dir.
const HTTP_CLIENT_STD_HNL: &str =
    include_str!("../../../kit-stdlib/data/http_client_std/http_client_std.hnl");

/// Build the [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) for
/// http-client-std, registering the Rust handls (wrapping [`StdHttpClient`])
/// under the op names declared in the http-client interface.
pub fn http_client_std_op_registry() -> handl_plugin_lib_handl::OpRegistry {
    http_client_op_registry(StdHttpClient)
}

// ---------------------------------------------------------------------------
// HttpClientStdLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `http-client-std` implementation
/// via the generic [`HandlLibraryPlugin`] with an
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
pub struct HttpClientStdLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl HttpClientStdLibraryPlugin {
    /// Create a new `HttpClientStdLibraryPlugin` backed by [`StdHttpClient`].
    pub fn new() -> Self {
        let registry = Arc::new(http_client_std_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "http",
                "lib.http-client-std",
                "lib-http-client-std",
                HTTP_CLIENT_STD_HNL,
                &[],
            )
            .with_ops(HTTP_CLIENT_OPS, registry),
        }
    }
}

impl Default for HttpClientStdLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for HttpClientStdLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for http-client-std with the op registry
/// (backed by [`StdHttpClient`]).
pub fn http_client_std_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(http_client_std_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "http",
        "lib.http-client-std",
        "lib-http-client-std",
        HTTP_CLIENT_STD_HNL,
        &[],
    )
    .with_ops(HTTP_CLIENT_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`HttpClientStdLibraryPlugin`].
pub use HttpClientStdLibraryPlugin as LibraryPlugin;
