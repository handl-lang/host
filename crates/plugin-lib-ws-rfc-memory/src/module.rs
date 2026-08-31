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
use handl_plugin_lib_ws_rfc::{
    ClientClose, ClientConnect, ClientNew, ClientRecv, ClientSend, FrameDecode, FrameEncode,
    MessageFromFrame, MessageToFrame, ServerListen, ServerNew, ServerStop,
};

/// The `ws_rfc` runtime module — WebSocket frame, message and in-process
/// client/server primitives.
#[derive(Clone, Copy)]
pub struct WsRfcModule;

impl Module for WsRfcModule {
    fn name(&self) -> &str {
        "ws_rfc"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "client" => Some(&ClientNew),
            "connect" => Some(&ClientConnect),
            "send" => Some(&ClientSend),
            "recv" => Some(&ClientRecv),
            "close" => Some(&ClientClose),
            "server" => Some(&ServerNew),
            "listen" => Some(&ServerListen),
            "stop" => Some(&ServerStop),
            "frame.encode" => Some(&FrameEncode),
            "frame.decode" => Some(&FrameDecode),
            "message.from_frame" => Some(&MessageFromFrame),
            "message.to_frame" => Some(&MessageToFrame),
            _ => None,
        }
    }
}
