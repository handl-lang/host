//! The `ws_rfc` handl runtime module.
//!
//! Exposes WebSocket RFC 6455 framing and a basic client/server API to handl
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
//!
//! The client/server implementation is currently an in-memory, single-process
//! transport. Real TCP/TLS networking will be added later.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_platform::{EvalContext, EvalError, Function};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::{CloseCode, CloseFrame, Frame, Message, OpCode};

static INNER: OnceLock<Arc<Mutex<Inner>>> = OnceLock::new();

fn inner() -> Arc<Mutex<Inner>> {
    INNER
        .get_or_init(|| Arc::new(Mutex::new(Inner::new())))
        .clone()
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

/// WebSocket frame and message helpers.
///
/// Reusable `Function` implementations used by the `ws_rfc` and
/// `ws_rfc_native` runtime modules.

fn parse_opcode(s: &str) -> Result<OpCode, EvalError> {
    match s {
        "text" => Ok(OpCode::Text),
        "binary" => Ok(OpCode::Binary),
        "close" => Ok(OpCode::Close),
        "ping" => Ok(OpCode::Ping),
        "pong" => Ok(OpCode::Pong),
        _ => Err(EvalError::Other(format!("unknown websocket opcode: {s}"))),
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

fn build_frame(obj: &serde_json::Map<String, JsonValue>) -> Result<Frame, EvalError> {
    let opcode = obj
        .get("opcode")
        .and_then(|v| v.as_str())
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))
        .and_then(parse_opcode)?;

    let payload_b64 = obj.get("payload").and_then(|v| v.as_str()).unwrap_or("");
    let payload = BASE64_STANDARD
        .decode(payload_b64)
        .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;

    let fin = obj
        .get("fin")
        .map(|v| {
            v.as_bool().ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "bool", "non-bool",
            )))
        })
        .unwrap_or(Ok(true))?;

    let mask = match obj.get("mask") {
        Some(v) if v.is_null() => None,
        Some(v) => {
            let s = v.as_str().ok_or(EvalError::TypeMismatch(format!(
                "expected {}, got {}",
                "string", "non-string",
            )))?;
            let bytes = BASE64_STANDARD
                .decode(s)
                .map_err(|e| EvalError::Other(format!("mask base64 decode error: {e}")))?;
            let bytes: [u8; 4] = bytes
                .try_into()
                .map_err(|_| EvalError::Other("mask must be exactly 4 bytes".to_string()))?;
            Some(bytes)
        }
        None => None,
    };

    Ok(Frame::new(opcode, payload)
        .with_fin(fin)
        .with_mask_opt(mask))
}

fn require_one_object(
    args: &[serde_json::Value],
) -> Result<&serde_json::Map<String, JsonValue>, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        1,
        args.len(),
    )))?;
    arg.as_object().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "object", "non-object",
    )))
}

fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        1,
        args.len(),
    )))?;
    arg.as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))
}

fn require_string_at(args: &[serde_json::Value], idx: usize) -> Result<&str, EvalError> {
    let arg = args.get(idx).ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        idx + 1,
        args.len(),
    )))?;
    arg.as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))
}

fn require_value_at(
    args: &[serde_json::Value],
    idx: usize,
) -> Result<&serde_json::Value, EvalError> {
    args.get(idx).ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        idx + 1,
        args.len(),
    )))
}

fn parse_u16(value: &JsonValue) -> Result<u16, EvalError> {
    let n = value.as_f64().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "number", "non-number",
    )))?;
    if n < 0.0 || n.fract() != 0.0 || n > u16::MAX as f64 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "u16", "invalid integer",
        )));
    }
    Ok(n as u16)
}

fn parse_id(value: &serde_json::Value, key: &str) -> Result<usize, EvalError> {
    if let Some(n) = value.as_f64() {
        return Ok(n as usize);
    }
    let obj = value.as_object().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "number or object", "non-number non-object",
    )))?;
    let id = obj
        .get(key)
        .ok_or_else(|| EvalError::Other(format!("missing {key}")))?;
    let n = id.as_f64().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "number", "non-number",
    )))?;
    Ok(n as usize)
}

#[derive(Clone, Copy)]
pub struct ClientNew;

impl Function for ClientNew {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                0,
                args.len(),
            )));
        }

        let inner = inner();
        let id = {
            let guard = inner.lock().unwrap();
            let id = guard.next_id();
            guard.clients.lock().unwrap().insert(id, Client::new());
            id
        };

        Ok(serde_json::Value::from(serde_json::json!({
            "type": "websocket_client",
            "client_id": id,
        })))
    }
}

#[derive(Clone, Copy)]
pub struct ClientConnect;

impl Function for ClientConnect {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }

        let id = parse_id(&args[0], "client_id")?;
        let url = require_string_at(args, 1)?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut clients = guard.clients.lock().unwrap();
        let client = clients
            .get_mut(&id)
            .ok_or(EvalError::Other(format!("unknown websocket client: {id}")))?;
        client.url = url.to_string();
        client.connected = true;

        Ok(serde_json::Value::from(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
pub struct ClientSend;

impl Function for ClientSend {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }

        let id = parse_id(&args[0], "client_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut clients = guard.clients.lock().unwrap();
        let client = clients
            .get_mut(&id)
            .ok_or(EvalError::Other(format!("unknown websocket client: {id}")))?;

        client.queue.push_back(args[1].clone());

        Ok(serde_json::Value::from(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
pub struct ClientRecv;

impl Function for ClientRecv {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?, "client_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut clients = guard.clients.lock().unwrap();
        let client = clients
            .get_mut(&id)
            .ok_or(EvalError::Other(format!("unknown websocket client: {id}")))?;

        let value = client.queue.pop_front().unwrap_or(JsonValue::Null);
        Ok(serde_json::Value::from(value))
    }
}

#[derive(Clone, Copy)]
pub struct ClientClose;

impl Function for ClientClose {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?, "client_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        guard.clients.lock().unwrap().remove(&id);

        Ok(serde_json::Value::from(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
pub struct ServerNew;

impl Function for ServerNew {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                0,
                args.len(),
            )));
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

        Ok(serde_json::Value::from(serde_json::json!({
            "type": "websocket_server",
            "server_id": id,
        })))
    }
}

#[derive(Clone, Copy)]
pub struct ServerListen;

impl Function for ServerListen {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )));
        }

        let id = parse_id(&args[0], "server_id")?;
        let addr = require_string_at(args, 1)?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut servers = guard.servers.lock().unwrap();
        let server = servers
            .get_mut(&id)
            .ok_or(EvalError::Other(format!("unknown websocket server: {id}")))?;
        server.addr = addr.to_string();
        server.handler = Some(args[2].clone());

        let mut addr_map = guard.addr_to_server.lock().unwrap();
        addr_map.insert(addr.to_string(), id);

        Ok(serde_json::Value::from(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
pub struct ServerStop;

impl Function for ServerStop {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?, "server_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let server = guard.servers.lock().unwrap().remove(&id);
        if let Some(server) = server {
            let mut addr_map = guard.addr_to_server.lock().unwrap();
            addr_map.remove(&server.addr);
        }

        Ok(serde_json::Value::from(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
pub struct FrameEncode;

impl Function for FrameEncode {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let obj = require_one_object(args)?;
        let frame = build_frame(obj)?;
        Ok(serde_json::Value::from(
            BASE64_STANDARD.encode(&frame.encode()),
        ))
    }
}

#[derive(Clone, Copy)]
pub struct FrameDecode;

impl Function for FrameDecode {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let s = require_one_string(args)?;
        let bytes = BASE64_STANDARD
            .decode(s)
            .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
        let (frame, _) = Frame::decode(&bytes)
            .map_err(|e| EvalError::Other(format!("frame decode error: {e}")))?;

        let mut obj = serde_json::Map::new();
        obj.insert(
            "opcode".to_string(),
            JsonValue::String(opcode_to_string(frame.opcode).to_string()),
        );
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
        Ok(serde_json::Value::from(JsonValue::Object(obj)))
    }
}

#[derive(Clone, Copy)]
pub struct MessageFromFrame;

impl Function for MessageFromFrame {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let obj = require_one_object(args)?;
        let frame = build_frame(obj)?;
        let message = Message::from_frame(&frame)
            .map_err(|e| EvalError::Other(format!("message from frame error: {e}")))?;

        let value = match message {
            Message::Text(text) => serde_json::json!({ "text": text }),
            Message::Binary(bytes) => {
                serde_json::json!({ "binary": BASE64_STANDARD.encode(&bytes) })
            }
            Message::Ping(bytes) => serde_json::json!({ "ping": BASE64_STANDARD.encode(&bytes) }),
            Message::Pong(bytes) => serde_json::json!({ "pong": BASE64_STANDARD.encode(&bytes) }),
            Message::Close(Some(frame)) => {
                serde_json::json!({ "close": { "code": frame.code.0, "reason": frame.reason } })
            }
            Message::Close(None) => serde_json::json!({ "close": null }),
        };
        Ok(serde_json::Value::from(value))
    }
}

#[derive(Clone, Copy)]
pub struct MessageToFrame;

impl Function for MessageToFrame {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let obj = require_one_object(args)?;
        if obj.len() != 1 {
            return Err(EvalError::Other(
                "message object must have exactly one key".to_string(),
            ));
        }

        let key = obj.keys().next().unwrap();
        let value = obj.get(key).unwrap();
        let message = match key.as_str() {
            "text" => {
                let text = value.as_str().ok_or(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "string", "non-string",
                )))?;
                Message::Text(text.to_string())
            }
            "binary" => {
                let s = value.as_str().ok_or(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "string", "non-string",
                )))?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
                Message::Binary(bytes)
            }
            "ping" => {
                let s = value.as_str().ok_or(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "string", "non-string",
                )))?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
                Message::Ping(bytes)
            }
            "pong" => {
                let s = value.as_str().ok_or(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "string", "non-string",
                )))?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
                Message::Pong(bytes)
            }
            "close" => {
                if value.is_null() {
                    Message::Close(None)
                } else {
                    let close_obj = value.as_object().ok_or(EvalError::TypeMismatch(format!(
                        "expected {}, got {}",
                        "object", "non-object",
                    )))?;
                    let code = parse_u16(close_obj.get("code").ok_or(EvalError::TypeMismatch(
                        format!("expected {}, got {}", "number", "missing",),
                    ))?)?;
                    let reason = close_obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Message::Close(Some(CloseFrame::new(CloseCode(code), reason)))
                }
            }
            _ => {
                return Err(EvalError::Other(format!("unknown message type: {key}")));
            }
        };

        let frame = message.into_frame(None);
        Ok(serde_json::Value::from(
            BASE64_STANDARD.encode(&frame.encode()),
        ))
    }
}
