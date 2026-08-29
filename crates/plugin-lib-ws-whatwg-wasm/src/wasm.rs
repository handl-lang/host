//! Browser `WebSocket` implementation for `websocket_whatwg` on `wasm32`.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{CanonicalContext, Error, NonFunctionValue};
use js_sys::{ArrayBuffer, Uint8Array};
use serde_json::Value as JsonValue;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
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
    queue: Rc<RefCell<VecDeque<NonFunctionValue>>>,
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

fn to_js_value(value: &JsonValue) -> Result<JsValue, Error> {
    if let Some(text) = value.as_str() {
        return Ok(JsValue::from_str(text));
    }
    let obj = value.as_object().ok_or(Error::TypeMismatch {
        expected: "string or object",
        got: "non-string non-object",
    })?;
    if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
        return Ok(JsValue::from_str(text));
    }
    if let Some(b64) = obj.get("binary").and_then(|v| v.as_str()) {
        let bytes = BASE64_STANDARD
            .decode(b64)
            .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
        return Ok(JsValue::from(Uint8Array::from(&bytes[..])));
    }
    Err(Error::PrimitiveError(
        "websocket message must be a string, {text}, or {binary}".to_string(),
    ))
}

fn from_js_value(data: JsValue) -> NonFunctionValue {
    if let Some(s) = data.as_string() {
        return NonFunctionValue::new(serde_json::json!({ "text": s }));
    }
    if let Some(ab) = data.dyn_ref::<ArrayBuffer>() {
        let arr = Uint8Array::new(ab);
        let mut bytes = vec![0; arr.length() as usize];
        arr.copy_to(&mut bytes);
        return NonFunctionValue::new(serde_json::json!({
            "binary": BASE64_STANDARD.encode(&bytes)
        }));
    }
    if let Some(blob) = data.dyn_ref::<web_sys::Blob>() {
        // Blobs cannot be read synchronously in wasm; emit a placeholder.
        return NonFunctionValue::new(serde_json::json!({
            "blob": format!("Blob(size={})", blob.size())
        }));
    }
    NonFunctionValue::new(serde_json::json!({ "unknown": format!("{:?}", data) }))
}

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
pub struct ConnectionConnect;

impl Function<NonFunctionValue> for ConnectionConnect {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let url = require_string_at(args, 0)?;

        let ws = WebSocket::new(url).map_err(|e| {
            Error::PrimitiveError(format!(
                "WebSocket constructor failed: {}",
                e.as_string().unwrap_or_default()
            ))
        })?;
        ws.set_binary_type(BinaryType::Arraybuffer);

        let open = Rc::new(Cell::new(false));
        let queue: Rc<RefCell<VecDeque<NonFunctionValue>>> = Rc::new(RefCell::new(VecDeque::new()));

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

        Ok(NonFunctionValue::new(serde_json::json!({
            "type": "websocket_connection",
            "connection_id": id,
        })))
    }
}

#[derive(Clone, Copy)]
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
        let js_value = to_js_value(args[1].inner())?;

        with_inner(|inner| {
            let connections = inner.connections.borrow();
            let conn = connections.get(&id).ok_or(Error::PrimitiveError(format!(
                "unknown websocket connection: {id}"
            )))?;

            if !conn.open.get() || conn.ws.ready_state() != WebSocket::OPEN {
                return Err(Error::PrimitiveError("websocket is not open".to_string()));
            }

            if let Some(s) = js_value.as_string() {
                conn.ws
                    .send_with_str(&s)
                    .map_err(|e| Error::PrimitiveError(format!("send failed: {e:?}")))?;
            } else if let Some(arr) = js_value.dyn_ref::<Uint8Array>() {
                conn.ws
                    .send_with_array_buffer_view(arr)
                    .map_err(|e| Error::PrimitiveError(format!("send failed: {e:?}")))?;
            } else {
                return Err(Error::PrimitiveError("unsupported message type".to_string()));
            }

            Ok(NonFunctionValue::new(JsonValue::Null))
        })
    }
}

#[derive(Clone, Copy)]
pub struct ConnectionRecv;

impl Function<NonFunctionValue> for ConnectionRecv {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = parse_id(require_value_at(args, 0)?)?;

        with_inner(|inner| {
            let connections = inner.connections.borrow();
            let conn = connections.get(&id).ok_or(Error::PrimitiveError(format!(
                "unknown websocket connection: {id}"
            )))?;

            let value = conn
                .queue
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| NonFunctionValue::new(JsonValue::Null));
            Ok(value)
        })
    }
}

#[derive(Clone, Copy)]
pub struct ConnectionClose;

impl Function<NonFunctionValue> for ConnectionClose {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = parse_id(require_value_at(args, 0)?)?;

        with_inner(|inner| {
            let mut connections = inner.connections.borrow_mut();
            if let Some(conn) = connections.remove(&id) {
                let _ = conn.ws.close();
            }
            Ok(NonFunctionValue::new(JsonValue::Null))
        })
    }
}

#[derive(Clone, Copy)]
pub struct WsWhatwgWasmModule;

impl Module for WsWhatwgWasmModule {
    fn name(&self) -> &str {
        "ws_whatwg_wasm"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
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
    let mut ctx = CanonicalContext::new();
    let value = ConnectionConnect
        .call(&[NonFunctionValue::new(JsonValue::String(url.to_string()))], &mut ctx)
        .map_err(js_err)?;
    let json = serde_json::to_string(value.inner()).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(JsValue::from_str(&json))
}

#[wasm_bindgen]
pub fn ws_whatwg_wasm_send(connection_id: usize, msg_json: &str) -> Result<(), JsValue> {
    let msg: JsonValue =
        serde_json::from_str(msg_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mut ctx = CanonicalContext::new();
    ConnectionSend
        .call(
            &[
                NonFunctionValue::new(JsonValue::Number(serde_json::Number::from(connection_id))),
                NonFunctionValue::new(msg),
            ],
            &mut ctx,
        )
        .map_err(js_err)?;
    Ok(())
}

#[wasm_bindgen]
pub fn ws_whatwg_wasm_recv(connection_id: usize) -> Result<JsValue, JsValue> {
    let mut ctx = CanonicalContext::new();
    let value = ConnectionRecv
        .call(
            &[NonFunctionValue::new(JsonValue::Number(serde_json::Number::from(connection_id)))],
            &mut ctx,
        )
        .map_err(js_err)?;
    let json = serde_json::to_string(value.inner()).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(JsValue::from_str(&json))
}

#[wasm_bindgen]
pub fn ws_whatwg_wasm_close(connection_id: usize) -> Result<(), JsValue> {
    let mut ctx = CanonicalContext::new();
    ConnectionClose
        .call(
            &[NonFunctionValue::new(JsonValue::Number(serde_json::Number::from(connection_id)))],
            &mut ctx,
        )
        .map_err(js_err)?;
    Ok(())
}
