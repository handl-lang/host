//! Native RFC 6455 WebSocket client/server for the handl runtime.
//!
//! This crate provides a real `tokio-tungstenite` backend for the
//! `ws_rfc_native` runtime module. It is intended for the `vm-server` and
//! other native hosts; sandboxed `vm-config` hosts should continue to use the
//! in-memory `handl-lib-ws-rfc` module.

mod function_value;
mod module;

pub use module::WsRfcNativeModule;

pub mod plugin;
pub use plugin::LibraryPlugin;
