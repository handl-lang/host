//! Native (non-wasm) `websocket_whatwg` client using `tokio-tungstenite`.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use futures_util::{SinkExt, StreamExt};
use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use tokio::runtime::Runtime;
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

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
    rx: Arc<Mutex<mpsc::Receiver<serde_json::Value>>>,
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

fn parse_id(value: &serde_json::Value) -> Result<usize, EvalError> {
    if let Some(n) = value.as_f64() {
        return Ok(n as usize);
    }
    let obj = value.as_object().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "number or object", "non-number non-object",
    )))?;
    let id = obj
        .get("connection_id")
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "connection_id", "missing",
        )))?;
    let n = id.as_f64().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "number", "non-number",
    )))?;
    Ok(n as usize)
}

fn tungstenite_to_json(msg: TungsteniteMessage) -> JsonValue {
    match msg {
        TungsteniteMessage::Text(t) => serde_json::json!({ "text": t }),
        TungsteniteMessage::Binary(b) => {
            serde_json::json!({ "binary": BASE64_STANDARD.encode(&b) })
        }
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

fn json_to_tungstenite(value: &JsonValue) -> Result<TungsteniteMessage, EvalError> {
    if let Some(text) = value.as_str() {
        return Ok(TungsteniteMessage::Text(text.to_string()));
    }
    let obj = value.as_object().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string or object", "non-string non-object",
    )))?;
    if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
        return Ok(TungsteniteMessage::Text(text.to_string()));
    }
    if let Some(b64) = obj.get("binary").and_then(|v| v.as_str()) {
        let bytes = BASE64_STANDARD
            .decode(b64)
            .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
        return Ok(TungsteniteMessage::Binary(bytes));
    }
    if let Some(b64) = obj.get("ping").and_then(|v| v.as_str()) {
        let bytes = BASE64_STANDARD
            .decode(b64)
            .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
        return Ok(TungsteniteMessage::Ping(bytes));
    }
    if let Some(b64) = obj.get("pong").and_then(|v| v.as_str()) {
        let bytes = BASE64_STANDARD
            .decode(b64)
            .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
        return Ok(TungsteniteMessage::Pong(bytes));
    }
    if let Some(close) = obj.get("close") {
        if close.is_null() {
            return Ok(TungsteniteMessage::Close(None));
        }
        let close_obj = close.as_object().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "object", "non-object",
        )))?;
        let code =
            close_obj
                .get("code")
                .and_then(|v| v.as_u64())
                .ok_or(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "u16", "non-number",
                )))?;
        let code = u16::try_from(code).map_err(|_| {
            EvalError::TypeMismatch(format!("expected {}, got {}", "u16", "out of range",))
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
    Err(EvalError::Other(
        "websocket message must be a string, {text}, {binary}, {ping}, {pong}, or {close}"
            .to_string(),
    ))
}

#[derive(Debug, Clone, Copy)]
pub struct WebsocketWhatwgName;

impl Function for WebsocketWhatwgName {
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

        Ok(serde_json::Value::from(serde_json::json!({
            "type": "websocket",
            "name": "websocket_whatwg",
        })))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionConnect;

impl Function for ConnectionConnect {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let url = require_string_at(args, 0)?;

        let (std_tx, std_rx) = mpsc::channel::<serde_json::Value>();
        let (cmd_tx, mut cmd_rx) = tokio_mpsc::unbounded_channel::<TungsteniteMessage>();

        let url_string = url.to_string();
        let connect_result = runtime().block_on(tokio_tungstenite::connect_async(&url_string));
        let (ws_stream, _) =
            connect_result.map_err(|e| EvalError::Other(format!("connect failed: {e}")))?;

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
                            let value = serde_json::Value::from(tungstenite_to_json(msg));
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

        Ok(serde_json::Value::from(serde_json::json!({
            "type": "websocket_connection",
            "connection_id": id,
        })))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionSend;

impl Function for ConnectionSend {
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

        let id = parse_id(&args[0])?;
        let msg = json_to_tungstenite(&args[1])?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let connections = guard.connections.lock().unwrap();
        let conn = connections.get(&id).ok_or(EvalError::Other(format!(
            "unknown websocket connection: {id}"
        )))?;
        conn.tx
            .send(msg)
            .map_err(|_| EvalError::Other("websocket connection is closed".to_string()))?;

        Ok(serde_json::Value::from(JsonValue::Null))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionRecv;

impl Function for ConnectionRecv {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?)?;

        let rx = {
            let inner = inner();
            let guard = inner.lock().unwrap();
            let connections = guard.connections.lock().unwrap();
            let conn = connections.get(&id).ok_or(EvalError::Other(format!(
                "unknown websocket connection: {id}"
            )))?;
            conn.rx.clone()
        };

        let value = rx
            .lock()
            .unwrap()
            .recv()
            .unwrap_or_else(|_| serde_json::Value::from(JsonValue::Null));
        Ok(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionClose;

impl Function for ConnectionClose {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?)?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut connections = guard.connections.lock().unwrap();
        if let Some(conn) = connections.remove(&id) {
            let _ = conn.tx.send(TungsteniteMessage::Close(None));
        }

        Ok(serde_json::Value::from(JsonValue::Null))
    }
}

#[derive(Clone, Copy)]
pub struct WsWhatwgNativeModule;

impl Module for WsWhatwgNativeModule {
    fn name(&self) -> &str {
        "ws_whatwg_native"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
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
