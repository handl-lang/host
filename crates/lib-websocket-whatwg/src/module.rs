//! The `websocket_whatwg` handl runtime module.
//!
//! On `wasm32` this wraps the browser `WebSocket` API. On all other targets
//! it falls back to an in-process self-echo queue.

#[cfg(target_arch = "wasm32")]
#[path = "wasm.rs"]
mod wasm;
#[cfg(target_arch = "wasm32")]
use wasm::*;

#[cfg(not(target_arch = "wasm32"))]
#[path = "native.rs"]
mod native;
#[cfg(not(target_arch = "wasm32"))]
use native::*;

use handl_vm::Function;
use handl_vm_common::Module;
use handl_vm_values_json::NonFunctionValue;

/// The `websocket_whatwg` runtime module.
#[derive(Debug, Clone, Copy)]
pub struct WebsocketWhatwgModule;

impl Module for WebsocketWhatwgModule {
    fn name(&self) -> &str {
        "websocket_whatwg"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "websocket_whatwg" => Some(&WebsocketWhatwgName),
            "connect" => Some(&ConnectionConnect),
            "send" => Some(&ConnectionSend),
            "recv" => Some(&ConnectionRecv),
            "close" => Some(&ConnectionClose),
            _ => None,
        }
    }
}
