//! The `ws_rfc` handl runtime module.
//!
//! Exposes an in-memory RFC 6455 WebSocket client/server API to handl
//! expressions:
//!
//! - `ws_rfc.client()` — create an RFC 6455 client
//! - `ws_rfc.connect(client, url)` — set the client URL
//! - `ws_rfc.send(client, msg)` — send a message
//! - `ws_rfc.recv(client)` — receive a message
//! - `ws_rfc.close(client)` — close the client
//! - `ws_rfc.server()` — create an RFC 6455 server
//! - `ws_rfc.listen(server, addr, handler)` — start listening
//! - `ws_rfc.stop(server)` — stop the server
//! - `ws_rfc.frame.encode(object)` — `base64` frame bytes
//! - `ws_rfc.frame.decode(base64)` — frame object
//! - `ws_rfc.message.from_frame(frame)` — message object
//! - `ws_rfc.message.to_frame(message)` — `base64` frame bytes

use handl_platform::Function;
use handl_platform::Module;
use handl_plugin_lib_handl::HandlModule;
use std::sync::OnceLock;

/// The handl source declaring the ws_rfc `op` interface.
const WS_RFC_MEMORY_HNL: &str =
    include_str!("../../../handl-packages/ws_rfc_memory/ws_rfc_memory.hnl");

/// A thread-safe lazy-initialized [`HandlModule`] for ws_rfc.
static WS_RFC_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for ws_rfc.
fn ws_rfc_module() -> &'static HandlModule {
    WS_RFC_MODULE.get_or_init(|| {
        let registry = std::sync::Arc::new(crate::ws_rfc_op_registry());
        HandlModule::new("ws_rfc", WS_RFC_MEMORY_HNL, &[]).with_ops(crate::WS_RFC_OPS, registry)
    })
}

/// The `ws_rfc` runtime module — WebSocket frame, message and in-process
/// client/server primitives.
///
/// The interface is declared in `ws_rfc_memory.hnl` via `op`/`handl`
/// declarations and loaded via the generic [`HandlModule`]; this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct WsRfcModule;

impl Module for WsRfcModule {
    fn name(&self) -> &str {
        "ws_rfc"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        // Try the full local name first (handles "client", "connect", etc.).
        if let Some(f) = ws_rfc_module().function(name) {
            return Some(f);
        }
        // HandlModule stores ops by their last segment (e.g. "encode" for
        // "ws_rfc.frame.encode"), but callers use the dotted local name
        // (e.g. "frame.encode"). Fall back to the last segment.
        if let Some((_, last)) = name.rsplit_once('.') {
            return ws_rfc_module().function(last);
        }
        None
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        ws_rfc_module().functions()
    }
}
