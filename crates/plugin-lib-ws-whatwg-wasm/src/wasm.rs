//! Browser `WebSocket` implementation for `websocket_whatwg` on `wasm32`.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use js_sys::{ArrayBuffer, Uint8Array};
use serde_json::Value as JsonValue;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

thread_local! {
    static INNER: RefCell<Inner> = RefCell::new(Inner::new());
}

fn with_inner<R>(f: impl FnOnce(&mut Inner) -> R) -> R {
    INNER.with(|inner| f(&mut *inner.borrow_mut()))
}

struct Connection {
    ws: WebSocket,
    open: Rc<Cell<bool>>,
    queue: Rc<RefCell<VecDeque<serde_json::Value>>>,
    #[allow(dead_code)]
    onopen: Closure<dyn FnMut()>,
    #[allow(dead_code)]
    onmessage: Closure<dyn FnMut(MessageEvent)>,
    #[allow(dead_code)]
    onclose: Closure<dyn FnMut(CloseEvent)>,
    #[allow(dead_code)]
    onerror: Closure<dyn FnMut(Event)>,
}

struct Inner {
    next_id: Cell<usize>,
    connections: RefCell<HashMap<usize, Connection>>,
}

impl Inner {
    fn new() -> Self {
        Self {
            next_id: Cell::new(1),
            connections: RefCell::new(HashMap::new()),
        }
    }

    fn next_id(&self) -> usize {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
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

fn to_js_value(value: &JsonValue) -> Result<JsValue, EvalError> {
    if let Some(text) = value.as_str() {
        return Ok(JsValue::from_str(text));
    }
    let obj = value.as_object().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string or object", "non-string non-object",
    )))?;
    if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
        return Ok(JsValue::from_str(text));
    }
    if let Some(b64) = obj.get("binary").and_then(|v| v.as_str()) {
        let bytes = BASE64_STANDARD
            .decode(b64)
            .map_err(|e| EvalError::Other(format!("base64 decode error: {e}")))?;
        return Ok(JsValue::from(Uint8Array::from(&bytes[..])));
    }
    Err(EvalError::Other(
        "websocket message must be a string, {text}, or {binary}".to_string(),
    ))
}

fn from_js_value(data: JsValue) -> serde_json::Value {
    if let Some(s) = data.as_string() {
        return serde_json::Value::from(serde_json::json!({ "text": s }));
    }
    if let Some(ab) = data.dyn_ref::<ArrayBuffer>() {
        let arr = Uint8Array::new(ab);
        let mut bytes = vec![0; arr.length() as usize];
        arr.copy_to(&mut bytes);
        return serde_json::Value::from(serde_json::json!({
            "binary": BASE64_STANDARD.encode(&bytes)
        }));
    }
    if let Some(blob) = data.dyn_ref::<web_sys::Blob>() {
        // Blobs cannot be read synchronously in wasm; emit a placeholder.
        return serde_json::Value::from(serde_json::json!({
            "blob": format!("Blob(size={})", blob.size())
        }));
    }
    serde_json::Value::from(serde_json::json!({ "unknown": format!("{:?}", data) }))
}

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
pub struct ConnectionConnect;

impl Function for ConnectionConnect {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let url = require_string_at(args, 0)?;

        let ws = WebSocket::new(url).map_err(|e| {
            EvalError::Other(format!(
                "WebSocket constructor failed: {}",
                e.as_string().unwrap_or_default()
            ))
        })?;
        ws.set_binary_type(BinaryType::Arraybuffer);

        let open = Rc::new(Cell::new(false));
        let queue: Rc<RefCell<VecDeque<serde_json::Value>>> =
            Rc::new(RefCell::new(VecDeque::new()));

        let open2 = open.clone();
        let onopen = Closure::new(move || {
            open2.set(true);
        });

        let queue2 = queue.clone();
        let onmessage = Closure::new(move |e: MessageEvent| {
            queue2.borrow_mut().push_back(from_js_value(e.data()));
        });

        let open3 = open.clone();
        let onclose = Closure::new(move |_: CloseEvent| {
            open3.set(false);
        });

        let onerror = Closure::new(move |_: Event| {});

        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        let conn = Connection {
            ws,
            open,
            queue,
            onopen,
            onmessage,
            onclose,
            onerror,
        };

        let id = with_inner(|inner| {
            let id = inner.next_id();
            inner.connections.borrow_mut().insert(id, conn);
            id
        });

        Ok(serde_json::Value::from(serde_json::json!({
            "type": "websocket_connection",
            "connection_id": id,
        })))
    }
}

#[derive(Clone, Copy)]
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
        let js_value = to_js_value(args[1])?;

        with_inner(|inner| {
            let connections = inner.connections.borrow();
            let conn = connections.get(&id).ok_or(EvalError::Other(format!(
                "unknown websocket connection: {id}"
            )))?;

            if !conn.open.get() || conn.ws.ready_state() != WebSocket::OPEN {
                return Err(EvalError::Other("websocket is not open".to_string()));
            }

            if let Some(s) = js_value.as_string() {
                conn.ws
                    .send_with_str(&s)
                    .map_err(|e| EvalError::Other(format!("send failed: {e:?}")))?;
            } else if let Some(arr) = js_value.dyn_ref::<Uint8Array>() {
                conn.ws
                    .send_with_array_buffer_view(arr)
                    .map_err(|e| EvalError::Other(format!("send failed: {e:?}")))?;
            } else {
                return Err(EvalError::Other("unsupported message type".to_string()));
            }

            Ok(serde_json::Value::from(JsonValue::Null))
        })
    }
}

#[derive(Clone, Copy)]
pub struct ConnectionRecv;

impl Function for ConnectionRecv {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?)?;

        with_inner(|inner| {
            let connections = inner.connections.borrow();
            let conn = connections.get(&id).ok_or(EvalError::Other(format!(
                "unknown websocket connection: {id}"
            )))?;

            let value = conn
                .queue
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| serde_json::Value::from(JsonValue::Null));
            Ok(value)
        })
    }
}

#[derive(Clone, Copy)]
pub struct ConnectionClose;

impl Function for ConnectionClose {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?)?;

        with_inner(|inner| {
            let mut connections = inner.connections.borrow_mut();
            if let Some(conn) = connections.remove(&id) {
                let _ = conn.ws.close();
            }
            Ok(serde_json::Value::from(JsonValue::Null))
        })
    }
}

#[derive(Clone, Copy)]
pub struct WsWhatwgWasmModule;

impl Module for WsWhatwgWasmModule {
    fn name(&self) -> &str {
        "ws_whatwg_wasm"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "ws_whatwg_wasm" => Some(&WebsocketWhatwgName),
            "connect" => Some(&ConnectionConnect),
            "send" => Some(&ConnectionSend),
            "recv" => Some(&ConnectionRecv),
            "close" => Some(&ConnectionClose),
            _ => None,
        }
    }
}

fn js_err(e: Error) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen]
pub fn ws_whatwg_wasm_connect(url: &str) -> Result<JsValue, JsValue> {
    let mut ctx = EvalContext::new();
    let value = ConnectionConnect
        .call(
            &[serde_json::Value::from(JsonValue::String(url.to_string()))],
            &mut ctx,
        )
        .map_err(js_err)?;
    let json = serde_json::to_string(value).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(JsValue::from_str(&json))
}

#[wasm_bindgen]
pub fn ws_whatwg_wasm_send(connection_id: usize, msg_json: &str) -> Result<(), JsValue> {
    let msg: JsonValue =
        serde_json::from_str(msg_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mut ctx = EvalContext::new();
    ConnectionSend
        .call(
            &[
                serde_json::Value::from(JsonValue::Number(serde_json::Number::from(connection_id))),
                serde_json::Value::from(msg),
            ],
            &mut ctx,
        )
        .map_err(js_err)?;
    Ok(())
}

#[wasm_bindgen]
pub fn ws_whatwg_wasm_recv(connection_id: usize) -> Result<JsValue, JsValue> {
    let mut ctx = EvalContext::new();
    let value = ConnectionRecv
        .call(
            &[serde_json::Value::from(JsonValue::Number(
                serde_json::Number::from(connection_id),
            ))],
            &mut ctx,
        )
        .map_err(js_err)?;
    let json = serde_json::to_string(value).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(JsValue::from_str(&json))
}

#[wasm_bindgen]
pub fn ws_whatwg_wasm_close(connection_id: usize) -> Result<(), JsValue> {
    let mut ctx = EvalContext::new();
    ConnectionClose
        .call(
            &[serde_json::Value::from(JsonValue::Number(
                serde_json::Number::from(connection_id),
            ))],
            &mut ctx,
        )
        .map_err(js_err)?;
    Ok(())
}
