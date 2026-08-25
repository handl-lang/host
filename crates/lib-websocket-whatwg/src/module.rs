//! The `websocket_whatwg` handl runtime module.
//!
//! Browser-side WebSocket client API. Currently this uses an in-memory,
//! single-process self-echo queue so that the handl API can be exercised
//! without a real browser host. Real `web-sys` integration will be added
//! later for the `wasm32` target.
//!
//! - `websocket_whatwg.connect(url)` — create a connection
//! - `websocket_whatwg.send(conn, msg)` — send a message
//! - `websocket_whatwg.recv(conn)` — receive a message
//! - `websocket_whatwg.close(conn)` — close the connection
//! - `websocket_whatwg.websocket_whatwg` — spec token

use handl_vm::{Context, Function};
use handl_vm_common::Module;
use handl_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

static INNER: OnceLock<Arc<Mutex<Inner>>> = OnceLock::new();

fn inner() -> Arc<Mutex<Inner>> {
    INNER.get_or_init(|| Arc::new(Mutex::new(Inner::new()))).clone()
}

struct Connection {
    queue: VecDeque<JsonValue>,
    open: bool,
}

impl Connection {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            open: true,
        }
    }
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

#[derive(Debug, Clone, Copy)]
struct WebsocketWhatwgName;

impl Function<NonFunctionValue> for WebsocketWhatwgName {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_vm::Error<NonFunctionValue>> {
        if !args.is_empty() {
            return Err(handl_vm::Error::ArgumentCount {
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
struct ConnectionConnect;

impl Function<NonFunctionValue> for ConnectionConnect {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let _url = require_string_at(args, 0)?;

        let inner = inner();
        let id = {
            let guard = inner.lock().unwrap();
            let id = guard.next_id();
            guard
                .connections
                .lock()
                .unwrap()
                .insert(id, Connection::new());
            id
        };

        Ok(NonFunctionValue::new(serde_json::json!({
            "type": "websocket_connection",
            "connection_id": id,
        })))
    }
}

#[derive(Debug, Clone, Copy)]
struct ConnectionSend;

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
        let value = &args[1];

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut connections = guard.connections.lock().unwrap();
        let conn = connections.get_mut(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket connection: {id}"
        )))?;

        if !conn.open {
            return Err(Error::PrimitiveError("websocket connection is closed".to_string()));
        }

        conn.queue.push_back(value.inner().clone());

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

#[derive(Debug, Clone, Copy)]
struct ConnectionRecv;

impl Function<NonFunctionValue> for ConnectionRecv {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let id = parse_id(require_value_at(args, 0)?)?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut connections = guard.connections.lock().unwrap();
        let conn = connections.get_mut(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket connection: {id}"
        )))?;

        let value = conn.queue.pop_front().unwrap_or(JsonValue::Null);
        Ok(NonFunctionValue::new(value))
    }
}

#[derive(Debug, Clone, Copy)]
struct ConnectionClose;

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
        if let Some(conn) = connections.get_mut(&id) {
            conn.open = false;
        }

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}
