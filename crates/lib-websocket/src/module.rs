//! The `websocket_rfc` handl runtime module.
//!
//! Exposes WebSocket RFC 6455 framing and a basic client/server API to handl
//! expressions:
//!
//! - `websocket_rfc.client()` — create an RFC 6455 client
//! - `websocket_rfc.connect(client, url)` — set the client URL
//! - `websocket_rfc.send(client, msg)` — send a message
//! - `websocket_rfc.recv(client)` — receive a message
//! - `websocket_rfc.close(client)` — close the client
//! - `websocket_rfc.server()` — create an RFC 6455 server
//! - `websocket_rfc.listen(server, addr, handler)` — start listening
//! - `websocket_rfc.stop(server)` — stop the server
//! - `websocket_rfc.frame.encode(object)` — `base64` frame bytes
//! - `websocket_rfc.frame.decode(base64)` — frame object
//! - `websocket_rfc.message.from_frame(frame)` — message object
//! - `websocket_rfc.message.to_frame(message)` — `base64` frame bytes
//!
//! The client/server implementation is currently an in-memory, single-process
//! transport. Real TCP/TLS networking will be added later.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_vm::{Context, Function};
use handl_vm_common::Module;
use handl_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::{CloseCode, CloseFrame, Frame, Message, OpCode};

static INNER: OnceLock<Arc<Mutex<Inner>>> = OnceLock::new();

fn inner() -> Arc<Mutex<Inner>> {
    INNER.get_or_init(|| Arc::new(Mutex::new(Inner::new()))).clone()
}

struct Client {
    url: String,
    queue: VecDeque<JsonValue>,
    connected: bool,
}

impl Client {
    fn new() -> Self {
        Self {
            url: String::new(),
            queue: VecDeque::new(),
            connected: false,
        }
    }
}

struct Server {
    addr: String,
    handler: Option<JsonValue>,
}

struct Inner {
    next_id: AtomicUsize,
    clients: Mutex<HashMap<usize, Client>>,
    servers: Mutex<HashMap<usize, Server>>,
    addr_to_server: Mutex<HashMap<String, usize>>,
}

impl Inner {
    fn new() -> Self {
        Self {
            next_id: AtomicUsize::new(1),
            clients: Mutex::new(HashMap::new()),
            servers: Mutex::new(HashMap::new()),
            addr_to_server: Mutex::new(HashMap::new()),
        }
    }

    fn next_id(&self) -> usize {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

/// The `websocket_rfc` module — WebSocket frame, message and client/server
/// primitives.
#[derive(Clone, Copy)]
pub struct WebsocketModule;

impl Module for WebsocketModule {
    fn name(&self) -> &str {
        "websocket_rfc"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
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

fn parse_opcode(s: &str) -> Result<OpCode, Error> {
    match s {
        "text" => Ok(OpCode::Text),
        "binary" => Ok(OpCode::Binary),
        "close" => Ok(OpCode::Close),
        "ping" => Ok(OpCode::Ping),
        "pong" => Ok(OpCode::Pong),
        _ => Err(Error::PrimitiveError(format!("unknown websocket opcode: {s}"))),
    }
}

fn opcode_to_string(op: OpCode) -> &'static str {
    match op {
        OpCode::Text => "text",
        OpCode::Binary => "binary",
        OpCode::Close => "close",
        OpCode::Ping => "ping",
        OpCode::Pong => "pong",
        OpCode::Continuation => "continuation",
    }
}

fn build_frame(obj: &serde_json::Map<String, JsonValue>) -> Result<Frame, Error> {
    let opcode = obj
        .get("opcode")
        .and_then(|v| v.as_str())
        .ok_or(Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        })
        .and_then(parse_opcode)?;

    let payload_b64 = obj.get("payload").and_then(|v| v.as_str()).unwrap_or("");
    let payload = BASE64_STANDARD
        .decode(payload_b64)
        .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;

    let fin = obj
        .get("fin")
        .map(|v| {
            v.as_bool().ok_or(Error::TypeMismatch {
                expected: "bool",
                got: "non-bool",
            })
        })
        .unwrap_or(Ok(true))?;

    let mask = match obj.get("mask") {
        Some(v) if v.is_null() => None,
        Some(v) => {
            let s = v.as_str().ok_or(Error::TypeMismatch {
                expected: "string",
                got: "non-string",
            })?;
            let bytes = BASE64_STANDARD
                .decode(s)
                .map_err(|e| Error::PrimitiveError(format!("mask base64 decode error: {e}")))?;
            let bytes: [u8; 4] = bytes.try_into().map_err(|_| {
                Error::PrimitiveError("mask must be exactly 4 bytes".to_string())
            })?;
            Some(bytes)
        }
        None => None,
    };

    Ok(Frame::new(opcode, payload).with_fin(fin).with_mask_opt(mask))
}

fn require_one_object(args: &[NonFunctionValue]) -> Result<&serde_json::Map<String, JsonValue>, Error> {
    let arg = args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: args.len(),
    })?;
    arg.as_object().ok_or(Error::TypeMismatch {
        expected: "object",
        got: "non-object",
    })
}

fn require_one_string(args: &[NonFunctionValue]) -> Result<&str, Error> {
    let arg = args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: args.len(),
    })?;
    arg.as_str().ok_or(Error::TypeMismatch {
        expected: "string",
        got: "non-string",
    })
}

fn require_string_at(args: &[NonFunctionValue], idx: usize) -> Result<&str, Error> {
    let arg = args.get(idx).ok_or(Error::ArgumentCount {
        expected: idx + 1,
        got: args.len(),
    })?;
    arg.as_str().ok_or(Error::TypeMismatch {
        expected: "string",
        got: "non-string",
    })
}

fn require_value_at(args: &[NonFunctionValue], idx: usize) -> Result<&NonFunctionValue, Error> {
    args.get(idx).ok_or(Error::ArgumentCount {
        expected: idx + 1,
        got: args.len(),
    })
}

fn parse_u16(value: &JsonValue) -> Result<u16, Error> {
    let n = value.as_f64().ok_or(Error::TypeMismatch {
        expected: "number",
        got: "non-number",
    })?;
    if n < 0.0 || n.fract() != 0.0 || n > u16::MAX as f64 {
        return Err(Error::TypeMismatch {
            expected: "u16",
            got: "invalid integer",
        });
    }
    Ok(n as u16)
}

fn parse_id(value: &NonFunctionValue, key: &str) -> Result<usize, Error> {
    if let Some(n) = value.as_number() {
        return Ok(n as usize);
    }
    let obj = value.as_object().ok_or(Error::TypeMismatch {
        expected: "number or object",
        got: "non-number non-object",
    })?;
    let id = obj
        .get(key)
        .ok_or_else(|| Error::PrimitiveError(format!("missing {key}")))?;
    let n = id.as_f64().ok_or(Error::TypeMismatch {
        expected: "number",
        got: "non-number",
    })?;
    Ok(n as usize)
}

#[derive(Clone, Copy)]
struct ClientNew;

impl Function<NonFunctionValue> for ClientNew {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if !args.is_empty() {
            return Err(Error::ArgumentCount {
                expected: 0,
                got: args.len(),
            });
        }

        let inner = inner();
        let id = {
            let guard = inner.lock().unwrap();
            let id = guard.next_id();
            guard
                .clients
                .lock()
                .unwrap()
                .insert(id, Client::new());
            id
        };

        Ok(NonFunctionValue::new(serde_json::json!({
            "type": "websocket_client",
            "client_id": id,
        })))
    }
}

#[derive(Clone, Copy)]
struct ClientConnect;

impl Function<NonFunctionValue> for ClientConnect {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() < 2 {
            return Err(Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }

        let id = parse_id(&args[0], "client_id")?;
        let url = require_string_at(args, 1)?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut clients = guard.clients.lock().unwrap();
        let client = clients.get_mut(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket client: {id}"
        )))?;
        client.url = url.to_string();
        client.connected = true;

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
struct ClientSend;

impl Function<NonFunctionValue> for ClientSend {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() < 2 {
            return Err(Error::ArgumentCount {
                expected: 2,
                got: args.len(),
            });
        }

        let id = parse_id(&args[0], "client_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut clients = guard.clients.lock().unwrap();
        let client = clients.get_mut(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket client: {id}"
        )))?;

        client.queue.push_back(args[1].inner().clone());

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
struct ClientRecv;

impl Function<NonFunctionValue> for ClientRecv {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = parse_id(require_value_at(args, 0)?, "client_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut clients = guard.clients.lock().unwrap();
        let client = clients.get_mut(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket client: {id}"
        )))?;

        let value = client.queue.pop_front().unwrap_or(JsonValue::Null);
        Ok(NonFunctionValue::new(value))
    }
}

#[derive(Clone, Copy)]
struct ClientClose;

impl Function<NonFunctionValue> for ClientClose {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = parse_id(require_value_at(args, 0)?, "client_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        guard.clients.lock().unwrap().remove(&id);

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
struct ServerNew;

impl Function<NonFunctionValue> for ServerNew {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if !args.is_empty() {
            return Err(Error::ArgumentCount {
                expected: 0,
                got: args.len(),
            });
        }

        let inner = inner();
        let id = {
            let guard = inner.lock().unwrap();
            let id = guard.next_id();
            guard.servers.lock().unwrap().insert(
                id,
                Server {
                    addr: String::new(),
                    handler: None,
                },
            );
            id
        };

        Ok(NonFunctionValue::new(serde_json::json!({
            "type": "websocket_server",
            "server_id": id,
        })))
    }
}

#[derive(Clone, Copy)]
struct ServerListen;

impl Function<NonFunctionValue> for ServerListen {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        if args.len() < 3 {
            return Err(Error::ArgumentCount {
                expected: 3,
                got: args.len(),
            });
        }

        let id = parse_id(&args[0], "server_id")?;
        let addr = require_string_at(args, 1)?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut servers = guard.servers.lock().unwrap();
        let server = servers.get_mut(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket server: {id}"
        )))?;
        server.addr = addr.to_string();
        server.handler = Some(args[2].inner().clone());

        let mut addr_map = guard.addr_to_server.lock().unwrap();
        addr_map.insert(addr.to_string(), id);

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
struct ServerStop;

impl Function<NonFunctionValue> for ServerStop {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = parse_id(require_value_at(args, 0)?, "server_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let server = guard.servers.lock().unwrap().remove(&id);
        if let Some(server) = server {
            let mut addr_map = guard.addr_to_server.lock().unwrap();
            addr_map.remove(&server.addr);
        }

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
struct FrameEncode;

impl Function<NonFunctionValue> for FrameEncode {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let obj = require_one_object(args)?;
        let frame = build_frame(obj)?;
        Ok(NonFunctionValue::new(BASE64_STANDARD.encode(&frame.encode())))
    }
}

#[derive(Clone, Copy)]
struct FrameDecode;

impl Function<NonFunctionValue> for FrameDecode {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let s = require_one_string(args)?;
        let bytes = BASE64_STANDARD
            .decode(s)
            .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
        let (frame, _) = Frame::decode(&bytes)
            .map_err(|e| Error::PrimitiveError(format!("frame decode error: {e}")))?;

        let mut obj = serde_json::Map::new();
        obj.insert("opcode".to_string(), JsonValue::String(opcode_to_string(frame.opcode).to_string()));
        obj.insert(
            "payload".to_string(),
            JsonValue::String(BASE64_STANDARD.encode(&frame.payload)),
        );
        obj.insert("fin".to_string(), JsonValue::Bool(frame.fin));
        obj.insert("rsv1".to_string(), JsonValue::Bool(frame.rsv1));
        obj.insert("rsv2".to_string(), JsonValue::Bool(frame.rsv2));
        obj.insert("rsv3".to_string(), JsonValue::Bool(frame.rsv3));
        if let Some(key) = frame.mask {
            obj.insert(
                "mask".to_string(),
                JsonValue::String(BASE64_STANDARD.encode(&key)),
            );
        }
        Ok(NonFunctionValue::new(JsonValue::Object(obj)))
    }
}

#[derive(Clone, Copy)]
struct MessageFromFrame;

impl Function<NonFunctionValue> for MessageFromFrame {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let obj = require_one_object(args)?;
        let frame = build_frame(obj)?;
        let message = Message::from_frame(&frame)
            .map_err(|e| Error::PrimitiveError(format!("message from frame error: {e}")))?;

        let value = match message {
            Message::Text(text) => serde_json::json!({ "text": text }),
            Message::Binary(bytes) => serde_json::json!({ "binary": BASE64_STANDARD.encode(&bytes) }),
            Message::Ping(bytes) => serde_json::json!({ "ping": BASE64_STANDARD.encode(&bytes) }),
            Message::Pong(bytes) => serde_json::json!({ "pong": BASE64_STANDARD.encode(&bytes) }),
            Message::Close(Some(frame)) => {
                serde_json::json!({ "close": { "code": frame.code.0, "reason": frame.reason } })
            }
            Message::Close(None) => serde_json::json!({ "close": null }),
        };
        Ok(NonFunctionValue::new(value))
    }
}

#[derive(Clone, Copy)]
struct MessageToFrame;

impl Function<NonFunctionValue> for MessageToFrame {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let obj = require_one_object(args)?;
        if obj.len() != 1 {
            return Err(Error::PrimitiveError(
                "message object must have exactly one key".to_string(),
            ));
        }

        let key = obj.keys().next().unwrap();
        let value = obj.get(key).unwrap();
        let message = match key.as_str() {
            "text" => {
                let text = value.as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: "non-string",
                })?;
                Message::Text(text.to_string())
            }
            "binary" => {
                let s = value.as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: "non-string",
                })?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
                Message::Binary(bytes)
            }
            "ping" => {
                let s = value.as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: "non-string",
                })?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
                Message::Ping(bytes)
            }
            "pong" => {
                let s = value.as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: "non-string",
                })?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
                Message::Pong(bytes)
            }
            "close" => {
                if value.is_null() {
                    Message::Close(None)
                } else {
                    let close_obj = value.as_object().ok_or(Error::TypeMismatch {
                        expected: "object",
                        got: "non-object",
                    })?;
                    let code = parse_u16(close_obj.get("code").ok_or(Error::TypeMismatch {
                        expected: "number",
                        got: "missing",
                    })?)?;
                    let reason = close_obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Message::Close(Some(CloseFrame::new(CloseCode(code), reason)))
                }
            }
            _ => {
                return Err(Error::PrimitiveError(format!(
                    "unknown message type: {key}"
                )))
            }
        };

        let frame = message.into_frame(None);
        Ok(NonFunctionValue::new(BASE64_STANDARD.encode(&frame.encode())))
    }
}
