#![forbid(unsafe_code)]
#![allow(missing_docs)]

//! In-memory RFC 6455 WebSocket client/server runtime module for the handl runtime.
//!
//! This crate provides the `ws_rfc` handl runtime module, which exposes the same
//! API as the real `ws_rfc_native` module but without any TCP/TLS I/O. It is
//! used by sandboxed hosts such as `vm-config` for testing and deterministic
//! evaluation.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The ws_rfc interface is declared in
//! `handl-packages/kit-stdlib/ws_rfc_memory/ws_rfc_memory.hnl` via `op` declarations,
//! and the Rust handls (re-exported from `handl_plugin_lib_ws_rfc`) are
//! registered in an [`OpRegistry`]. The [`WsRfcMemoryLibraryPlugin`] wraps
//! the generic [`HandlLibraryPlugin`] with the op registry.
//!
//! The [`WsRfcModule`] re-export is preserved for backward compatibility.

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use handl_plugin_lib_ws_rfc::{
    ClientClose, ClientConnect, ClientNew, ClientRecv, ClientSend, FrameDecode, FrameEncode,
    MessageFromFrame, MessageToFrame, ServerListen, ServerNew, ServerStop,
};

mod module;

pub use module::WsRfcModule;

#[cfg(test)]
mod tests;

/// The handl source declaring the ws_rfc `op` interface — the single source
/// of truth for the ws_rfc API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const WS_RFC_MEMORY_HNL: &str =
    include_str!("../../../handl-packages/kit-stdlib/ws_rfc_memory/ws_rfc_memory.hnl");

/// The op names declared in `ws_rfc_memory.hnl`.
const WS_RFC_OPS: &[&str] = &[
    "ws_rfc.client",
    "ws_rfc.connect",
    "ws_rfc.send",
    "ws_rfc.recv",
    "ws_rfc.close",
    "ws_rfc.server",
    "ws_rfc.listen",
    "ws_rfc.stop",
    "ws_rfc.frame.encode",
    "ws_rfc.frame.decode",
    "ws_rfc.message.from_frame",
    "ws_rfc.message.to_frame",
];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for ws_rfc, registering the Rust handls under
/// their op names.
pub fn ws_rfc_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("ws_rfc.client", Arc::new(ClientNew));
    registry.register("ws_rfc.connect", Arc::new(ClientConnect));
    registry.register("ws_rfc.send", Arc::new(ClientSend));
    registry.register("ws_rfc.recv", Arc::new(ClientRecv));
    registry.register("ws_rfc.close", Arc::new(ClientClose));
    registry.register("ws_rfc.server", Arc::new(ServerNew));
    registry.register("ws_rfc.listen", Arc::new(ServerListen));
    registry.register("ws_rfc.stop", Arc::new(ServerStop));
    registry.register("ws_rfc.frame.encode", Arc::new(FrameEncode));
    registry.register("ws_rfc.frame.decode", Arc::new(FrameDecode));
    registry.register("ws_rfc.message.from_frame", Arc::new(MessageFromFrame));
    registry.register("ws_rfc.message.to_frame", Arc::new(MessageToFrame));
    registry
}

// ---------------------------------------------------------------------------
// WsRfcMemoryLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `ws_rfc` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
pub struct WsRfcMemoryLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl WsRfcMemoryLibraryPlugin {
    /// Create a new `WsRfcMemoryLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(ws_rfc_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "ws_rfc",
                "lib.ws-rfc-memory",
                "lib-ws-rfc-memory",
                WS_RFC_MEMORY_HNL,
                &[],
            )
            .with_ops(WS_RFC_OPS, registry),
        }
    }
}

impl Default for WsRfcMemoryLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for WsRfcMemoryLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for ws_rfc with the op registry.
pub fn ws_rfc_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(ws_rfc_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "ws_rfc",
        "lib.ws-rfc-memory",
        "lib-ws-rfc-memory",
        WS_RFC_MEMORY_HNL,
        &[],
    )
    .with_ops(WS_RFC_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`WsRfcMemoryLibraryPlugin`].
pub use WsRfcMemoryLibraryPlugin as LibraryPlugin;
