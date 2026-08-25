//! # handl-lib-websocket-whatwg
//!
//! WHATWG WebSocket client module for `handl`.
//!
//! This crate exposes the browser-side `WebSocket` API to handl programs.
//! It is intended for use in the `wasm32` / `loader-web` target, where the
//! native WebSocket implementation is provided by the user agent.

pub mod module;

pub use module::WebsocketWhatwgModule;
