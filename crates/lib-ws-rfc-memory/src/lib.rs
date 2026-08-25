#![forbid(unsafe_code)]
#![allow(missing_docs)]

//! In-memory RFC 6455 WebSocket client/server runtime module for the handl runtime.
//!
//! This crate provides the `ws_rfc` handl runtime module, which exposes the same
//! API as the real `ws_rfc_native` module but without any TCP/TLS I/O. It is
//! used by sandboxed hosts such as `vm-config` for testing and deterministic
//! evaluation.

mod module;

pub use module::WsRfcModule;
