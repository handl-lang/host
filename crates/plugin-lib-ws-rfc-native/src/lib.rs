//! Native RFC 6455 WebSocket client/server for the handl runtime.
//!
//! This crate provides a real `tokio-tungstenite` backend for the
//! `ws_rfc_native` runtime module. It is intended for the `vm-server` and
//! other native hosts; sandboxed `vm-config` hosts should continue to use the
//! in-memory `handl-lib-ws-rfc` module.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The ws_rfc interface is declared in
//! `handl-packages/ws_rfc/ws_rfc.hnl`, and this crate's `handl` bindings
//! are declared in `handl-packages/ws_rfc_native/ws_rfc_native.hnl`. The
//! Rust handls (wrapping tokio-tungstenite) are registered in an
//! [`OpRegistry`] via [`ws_rfc_native_op_registry`]. The
//! [`WsRfcNativeLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`]
//! with the op registry, eliminating the per-library `Plugin` trait
//! scaffolding.

use std::sync::Arc;

use handl_plugin_lib_handl::HandlLibraryPlugin;

mod function_value;
mod module;

pub use module::{ws_rfc_native_op_registry, WsRfcNativeModule, WS_RFC_NATIVE_OPS};

/// The handl source declaring the ws_rfc_native `handl` bindings — the
/// implementation-side declarations for the ws_rfc ops. Loaded at compile
/// time from the kit-stdlib data dir.
const WS_RFC_NATIVE_HNL: &str =
    include_str!("../../../handl-packages/ws_rfc_native/ws_rfc_native.hnl");

// ---------------------------------------------------------------------------
// WsRfcNativeLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `ws_rfc_native` implementation
/// via the generic [`HandlLibraryPlugin`] with an
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The ws_rfc interface is declared in `ws_rfc.hnl`; this crate's `handl`
/// bindings are declared in `ws_rfc_native.hnl`. The Rust handls (wrapping
/// tokio-tungstenite) are registered in the op registry. The generic
/// loader handles module registration.
pub struct WsRfcNativeLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl WsRfcNativeLibraryPlugin {
    /// Create a new `WsRfcNativeLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(ws_rfc_native_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "ws_rfc",
                "lib.ws-rfc-native",
                "lib-ws-rfc-native",
                WS_RFC_NATIVE_HNL,
                &[],
            )
            .with_ops(WS_RFC_NATIVE_OPS, registry),
        }
    }
}

impl Default for WsRfcNativeLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for WsRfcNativeLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for ws_rfc_native with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`WsRfcNativeLibraryPlugin`].
pub fn ws_rfc_native_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(ws_rfc_native_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "ws_rfc",
        "lib.ws-rfc-native",
        "lib-ws-rfc-native",
        WS_RFC_NATIVE_HNL,
        &[],
    )
    .with_ops(WS_RFC_NATIVE_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`WsRfcNativeLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_ws_rfc_native::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
pub use WsRfcNativeLibraryPlugin as LibraryPlugin;
