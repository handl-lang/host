//! # handl-lib-tls
//!
//! Abstract TLS library for the handl runtime.
//!
//! Defines the [`TlsService`] trait and a [`TlsModule`] that exposes TLS
//! primitives through the VM. This crate is I/O-free and depends on a
//! concrete backend such as `handl-lib-tls-native` to perform real
//! certificate generation and network operations.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The TLS interface is declared in
//! `handl-packages/tls/tls.hnl` via `op` declarations, and the Rust
//! handls (handlers) that delegate to a [`TlsService`] are registered in
//! an [`OpRegistry`] via [`tls_op_registry`]. The [`TlsLibraryPlugin`]
//! wraps the generic [`HandlLibraryPlugin`] with the op registry (backed
//! by [`StubTls`] by default), eliminating the per-library `Plugin` trait
//! scaffolding.
//!
//! ## Architecture
//!
//! ```text
//! handl-plugin-vm-common          handl-lib-tls          handl-lib-tls-native
//! ─────────────            ─────────────          ────────────────────
//! Module trait             TlsService trait       NativeTls
//! ModuleRuntime               TlsModule              rcgen, rustls,
//!                          StubTls                tokio-rustls
//!                          TlsLibraryPlugin       TlsNativeLibraryPlugin
//! ```
//!
//! ## Functions
//!
//! - [`TlsModule`] — `tls.version`, `tls.generate_self_signed`,
//!   `tls.parse_cert`, `tls.fingerprint`, `tls.client_config`,
//!   `tls.server_config`, `tls.connect`, `tls.accept`, `tls.read`,
//!   `tls.write`, `tls.close_conn`, `tls.close_listener`

mod error;
mod module;
mod service;
mod stub;

use std::sync::Arc;

use handl_plugin_lib_handl::HandlLibraryPlugin;

pub use error::TlsError;
pub use module::{tls_op_registry, TlsModule, TLS_OPS};
pub use service::TlsService;
pub use stub::StubTls;

/// The handl source declaring the tls `op` interface — the single source
/// of truth for the TLS API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const TLS_HNL: &str = include_str!("../../../handl-packages/tls/tls.hnl");

// ---------------------------------------------------------------------------
// TlsLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `tls` interface via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The TLS interface is declared in `tls.hnl` via `op`/`handl`
/// declarations; the Rust handls (delegating to [`StubTls`] by default)
/// are registered in the op registry. The generic loader handles module
/// registration.
///
/// Concrete backends such as `handl-lib-tls-native` provide their own
/// plugin ([`TlsNativeLibraryPlugin`]) with a real [`TlsService`]
/// implementation.
pub struct TlsLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl TlsLibraryPlugin {
    /// Create a new `TlsLibraryPlugin` backed by [`StubTls`].
    pub fn new() -> Self {
        let registry = Arc::new(tls_op_registry(StubTls::new()));
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "tls",
                "lib.tls",
                "lib-tls",
                TLS_HNL,
                &[],
            )
            .with_ops(TLS_OPS, registry),
        }
    }
}

impl Default for TlsLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for TlsLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for tls with the op registry (backed by
/// [`StubTls`]).
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`TlsLibraryPlugin`].
pub fn tls_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(tls_op_registry(StubTls::new()));
    HandlLibraryPlugin::new_with_caps("tls", "lib.tls", "lib-tls", TLS_HNL, &[])
        .with_ops(TLS_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`TlsLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_tls::LibraryPlugin`.
pub use TlsLibraryPlugin as LibraryPlugin;
