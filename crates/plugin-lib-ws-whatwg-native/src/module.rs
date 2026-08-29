//! Native (non-wasm) `websocket_whatwg` client using `tokio-tungstenite`.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use futures_util::{SinkExt, StreamExt};
use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use tokio::runtime::Runtime;
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static INNER: OnceLock<Arc<Mutex<Inner>>> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn inner() -> Arc<Mutex<Inner>> {
    INNER
        .get_or_init(|| Arc::new(Mutex::new(Inner::new())))
        .clone()
}

struct Connection {
    tx: tokio_mpsc::UnboundedSender<TungsteniteMessage>,
    rx: Arc<Mutex<mpsc::Receiver<NonFunctionValue>>>,
}

struct Inner {
    next_id: AtomicUsize,
    connections: Mutex<HashMap<usize, Connection>>,
}

impl Inner {
    fn new() -> Self {
        Self {
            next_id: AtomicUsize::new(1),
            connections: Mutex::new(HashMap::new()),
        }
    }

    fn next_id(&self) -> usize {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
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

fn parse_id(value: &NonFunctionValue) -> Result<usize, Error> {
    if let Some(n) = value.as_number() {
        return Ok(n as usize);
    }
    let obj = value.as_object().ok_or(Error::TypeMismatch {
        expected: "number or object",
        got: "non-number non-object",
    })?;
    let id = obj.get("connection_id").ok_or(Error::TypeMismatch {
        expected: "connection_id",
        got: "missing",
    })?;
    let n = id.as_f64().ok_or(Error::TypeMismatch {
        expected: "number",
        got: "non-number",
    })?;
    Ok(n as usize)
}

fn tungstenite_to_json(msg: TungsteniteMessage) -> JsonValue {
    match msg {
        TungsteniteMessage::Text(t) => serde_json::json!({ "text": t }),
        TungsteniteMessage::Binary(b) => serde_json::json!({ "binary": BASE64_STANDARD.encode(&b) }),
        TungsteniteMessage::Ping(b) => serde_json::json!({ "ping": BASE64_STANDARD.encode(&b) }),
        TungsteniteMessage::Pong(b) => serde_json::json!({ "pong": BASE64_STANDARD.encode(&b) }),
        TungsteniteMessage::Close(cf) => match cf {
            Some(cf) => serde_json::json!({
                "close": {
                    "code": u16::from(cf.code),
                    "reason": cf.reason.as_ref(),
                }
            }),
            None => serde_json::json!({ "close": null }),
        },
        TungsteniteMessage::Frame(_) => serde_json::json!({ "frame": null }),
    }
}

fn json_to_tungstenite(value: &JsonValue) -> Result<TungsteniteMessage, Error> {
    if let Some(text) = value.as_str() {
        return Ok(TungsteniteMessage::Text(text.to_string()));
    }
    let obj = value.as_object().ok_or(Error::TypeMismatch {
        expected: "string or object",
        got: "non-string non-object",
    })?;
    if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
        return Ok(TungsteniteMessage::Text(text.to_string()));
    }
    if let Some(b64) = obj.get("binary").and_then(|v| v.as_str()) {
        let bytes = BASE64_STANDARD
            .decode(b64)
            .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
        return Ok(TungsteniteMessage::Binary(bytes));
    }
    if let Some(b64) = obj.get("ping").and_then(|v| v.as_str()) {
        let bytes = BASE64_STANDARD
            .decode(b64)
            .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
        return Ok(TungsteniteMessage::Ping(bytes));
    }
    if let Some(b64) = obj.get("pong").and_then(|v| v.as_str()) {
        let bytes = BASE64_STANDARD
            .decode(b64)
            .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
        return Ok(TungsteniteMessage::Pong(bytes));
    }
    if let Some(close) = obj.get("close") {
        if close.is_null() {
            return Ok(TungsteniteMessage::Close(None));
        }
        let close_obj = close.as_object().ok_or(Error::TypeMismatch {
            expected: "object",
            got: "non-object",
        })?;
        let code = close_obj
            .get("code")
            .and_then(|v| v.as_u64())
            .ok_or(Error::TypeMismatch {
                expected: "u16",
                got: "non-number",
            })?;
        let code = u16::try_from(code).map_err(|_| Error::TypeMismatch {
            expected: "u16",
            got: "out of range",
        })?;
        let reason = close_obj
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        return Ok(TungsteniteMessage::Close(Some(CloseFrame {
            code: code.into(),
            reason: reason.into(),
        })));
    }
    Err(Error::PrimitiveError(
        "websocket message must be a string, {text}, {binary}, {ping}, {pong}, or {close}".to_string(),
    ))
}

#[derive(Debug, Clone, Copy)]
pub struct WebsocketWhatwgName;

impl Function<NonFunctionValue> for WebsocketWhatwgName {
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

        Ok(NonFunctionValue::new(serde_json::json!({
            "type": "websocket",
            "name": "websocket_whatwg",
        })))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionConnect;

impl Function<NonFunctionValue> for ConnectionConnect {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let url = require_string_at(args, 0)?;

        let (std_tx, std_rx) = mpsc::channel::<NonFunctionValue>();
        let (cmd_tx, mut cmd_rx) = tokio_mpsc::unbounded_channel::<TungsteniteMessage>();

        let url_string = url.to_string();
        let connect_result =
            runtime().block_on(tokio_tungstenite::connect_async(&url_string));
        let (ws_stream, _) =
            connect_result.map_err(|e| Error::PrimitiveError(format!("connect failed: {e}")))?;

        let (mut write, mut read) = ws_stream.split();

        runtime().spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = cmd_rx.recv() => {
                        if write.send(msg).await.is_err() {
                            break;
                        }
                    }
                    next = read.next() => match next {
                        Some(Ok(msg)) => {
                            let value = NonFunctionValue::new(tungstenite_to_json(msg));
                            if std_tx.send(value).is_err() {
                                break;
                            }
                        }
                        Some(Err(_)) | None => break,
                    }
                }
            }
        });

        let inner = inner();
        let guard = inner.lock().unwrap();
        let id = guard.next_id();
        guard.connections.lock().unwrap().insert(
            id,
            Connection {
                tx: cmd_tx,
                rx: Arc::new(Mutex::new(std_rx)),
            },
        );

        Ok(NonFunctionValue::new(serde_json::json!({
            "type": "websocket_connection",
            "connection_id": id,
        })))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionSend;

impl Function<NonFunctionValue> for ConnectionSend {
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

        let id = parse_id(&args[0])?;
        let msg = json_to_tungstenite(args[1].inner())?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let connections = guard.connections.lock().unwrap();
        let conn = connections.get(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket connection: {id}"
        )))?;
        conn.tx
            .send(msg)
            .map_err(|_| Error::PrimitiveError("websocket connection is closed".to_string()))?;

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionRecv;

impl Function<NonFunctionValue> for ConnectionRecv {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = parse_id(require_value_at(args, 0)?)?;

        let rx = {
            let inner = inner();
            let guard = inner.lock().unwrap();
            let connections = guard.connections.lock().unwrap();
            let conn = connections.get(&id).ok_or(Error::PrimitiveError(format!(
                "unknown websocket connection: {id}"
            )))?;
            conn.rx.clone()
        };

        let value = rx
            .lock()
            .unwrap()
            .recv()
            .unwrap_or_else(|_| NonFunctionValue::new(JsonValue::Null));
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionClose;

impl Function<NonFunctionValue> for ConnectionClose {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = parse_id(require_value_at(args, 0)?)?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut connections = guard.connections.lock().unwrap();
        if let Some(conn) = connections.remove(&id) {
            let _ = conn.tx.send(TungsteniteMessage::Close(None));
        }

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
pub struct WsWhatwgNativeModule;

impl Module for WsWhatwgNativeModule {
    fn name(&self) -> &str {
        "ws_whatwg_native"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "ws_whatwg_native" => Some(&WebsocketWhatwgName),
            "connect" => Some(&ConnectionConnect),
            "send" => Some(&ConnectionSend),
            "recv" => Some(&ConnectionRecv),
            "close" => Some(&ConnectionClose),
            _ => None,
        }
    }
}
