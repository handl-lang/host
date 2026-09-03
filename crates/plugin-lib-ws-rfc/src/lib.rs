#![forbid(unsafe_code)]
#![allow(missing_docs)]

//! WebSocket frame and message codec.
//!
//! This crate provides a fast, dependency-free implementation of the WebSocket
//! framing protocol (RFC 6455). It is designed to be embedded in clients,
//! servers, and runtime modules that need to produce or consume WebSocket
//! frames without committing to a particular async runtime or TLS stack.
//!
//! It also exposes reusable `Function` helpers for `frame.encode`,
//! `frame.decode`, `message.from_frame`, and `message.to_frame`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The ws_rfc interface is declared in
//! `kit-stdlib/data/ws_rfc/ws_rfc.hnl` via `---` doc comments and `cap`
//! declarations. The [`WsRfcLibraryPlugin`] wraps a generic
//! [`HandlLibraryPlugin`] for the interface declaration (id, name,
//! capabilities). This is an interface crate; concrete implementations
//! are provided by `ws_rfc_memory` (in-memory) and `ws_rfc_native`
//! (tokio-tungstenite), which register the actual ops.
//!
//! # Example
//!
//! ```
//! use handl_plugin_lib_ws_rfc::{Frame, Message, OpCode};
//!
//! let frame = Frame::new(OpCode::Text, b"hello".to_vec());
//! let bytes = frame.encode();
//! let (decoded, _) = Frame::decode(&bytes).unwrap();
//! let message = Message::from_frame(&decoded).unwrap();
//! assert_eq!(message, Message::Text("hello".to_string()));
//! ```

use handl_plugin_lib_handl::HandlLibraryPlugin;

pub mod close;
pub mod error;
pub mod frame;
pub mod mask;
pub mod message;
pub mod module;
pub mod opcode;
pub mod reassembler;

pub use close::{CloseCode, CloseFrame};
pub use error::Error;
pub use frame::Frame;
pub use message::Message;
pub use module::{
    ClientClose, ClientConnect, ClientNew, ClientRecv, ClientSend, FrameDecode, FrameEncode,
    MessageFromFrame, MessageToFrame, ServerListen, ServerNew, ServerStop,
};
pub use opcode::OpCode;
pub use reassembler::Reassembler;

#[cfg(test)]
mod tests;

/// The handl source declaring the ws_rfc interface — the single source
/// of truth for the ws_rfc API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const WS_RFC_HNL: &str = include_str!("../../../kit-stdlib/data/ws_rfc/ws_rfc.hnl");

// ---------------------------------------------------------------------------
// WsRfcLibraryPlugin (wraps HandlLibraryPlugin for interface declaration)
// ---------------------------------------------------------------------------

/// A `Library` plugin that declares the `ws_rfc` interface via a
/// generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// This is an interface crate — it does not register any module. Concrete
/// implementations are provided by `ws_rfc_memory` and `ws_rfc_native`,
/// which register the actual ops via their own [`HandlLibraryPlugin`]
/// instances with op registries.
pub struct WsRfcLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl WsRfcLibraryPlugin {
    /// Create a new `WsRfcLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "ws_rfc",
                "lib.ws-rfc",
                "lib-ws-rfc",
                WS_RFC_HNL,
                &[],
            ),
        }
    }
}

impl Default for WsRfcLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for WsRfcLibraryPlugin {
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
        _capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        None
    }

    fn contribute(
        &self,
        _ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        Ok(())
    }

    fn run(
        &self,
        _request: &handl_platform::CapabilityRequest,
        _ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        Err(handl_platform::PlatformError::NotImplemented)
    }
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`WsRfcLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_ws_rfc::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
pub use WsRfcLibraryPlugin as LibraryPlugin;
