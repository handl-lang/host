//! The `websocket_rfc` handl runtime module backed by `tokio-tungstenite`.
//!
//! Provides real TCP/TLS WebSocket clients and servers plus the frame and
//! message helpers from `handl-lib-websocket`.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use futures_util::{SinkExt, StreamExt};
use handl_lib_websocket::module::{FrameDecode, FrameEncode, MessageFromFrame, MessageToFrame};
use handl_vm::{Context, Eval, Function};
use handl_vm_backend_rust_interpreter::CanonicalEval;
use handl_vm_common::Module;
use handl_vm_host_eval::EvalHost;
use handl_vm_values_json::{Error, FunctionValue, NonFunctionValue};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static INNER: OnceLock<Arc<Mutex<Inner>>> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime")
    })
}

fn inner() -> Arc<Mutex<Inner>> {
    INNER
        .get_or_init(|| Arc::new(Mutex::new(Inner::new())))
        .clone()
}

struct Client {
    url: String,
    tx: tokio_mpsc::UnboundedSender<TungsteniteMessage>,
    rx: Mutex<mpsc::Receiver<NonFunctionValue>>,
}

struct Server {
    addr: String,
    handler: Option<FunctionValue>,
    _incoming: Mutex<mpsc::Receiver<NonFunctionValue>>,
    _shutdown: Mutex<Option<oneshot::Sender<()>>>,
    _handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct Inner {
    next_id: AtomicUsize,
    clients: Mutex<HashMap<usize, Client>>,
    servers: Mutex<HashMap<usize, Server>>,
    eval: Arc<CanonicalEval<EvalHost>>,
}

impl Inner {
    fn new() -> Self {
        Self {
            next_id: AtomicUsize::new(1),
            clients: Mutex::new(HashMap::new()),
            servers: Mutex::new(HashMap::new()),
            eval: Arc::new(CanonicalEval::new(EvalHost::default(), None, None)),
        }
    }

    fn next_id(&self) -> usize {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

/// The `websocket_rfc` runtime module with a real `tokio-tungstenite`
/// client/server backend.
#[derive(Clone, Copy)]
pub struct WebsocketRfcStdModule;

impl Module for WebsocketRfcStdModule {
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
        TungsteniteMessage::Frame(f) => serde_json::json!({
            "frame": {
                "opcode": format!("{:?}", f.header().opcode),
                "payload": BASE64_STANDARD.encode(f.payload()),
                "fin": f.header().is_final,
            }
        }),
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
        "websocket message must be {text}, {binary}, {ping}, {pong}, or {close}".to_string(),
    ))
}

fn socket_addr(addr: &str) -> Result<SocketAddr, Error> {
    if let Ok(sa) = addr.parse::<SocketAddr>() {
        return Ok(sa);
    }
    let url = url::Url::parse(addr).map_err(|e| Error::PrimitiveError(format!("url parse: {e}")))?;
    let host = url
        .host_str()
        .ok_or(Error::PrimitiveError("missing host".to_string()))?;
    let port = url
        .port()
        .ok_or(Error::PrimitiveError("missing port".to_string()))?;
    let ip = (host, port)
        .to_socket_addrs()
        .map_err(|e| Error::PrimitiveError(format!("dns lookup: {e}")))?
        .next()
        .ok_or(Error::PrimitiveError("could not resolve address".to_string()))?;
    Ok(ip)
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
            guard.clients.lock().unwrap().insert(
                id,
                Client {
                    url: String::new(),
                    tx: tokio_mpsc::unbounded_channel().0,
                    rx: Mutex::new(mpsc::channel().1),
                },
            );
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
        let mut clients = guard.clients.lock().unwrap();
        let client = clients.get_mut(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket client: {id}"
        )))?;
        client.url = url_string;
        client.tx = cmd_tx;
        *client.rx.get_mut().unwrap() = std_rx;

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
        let msg = json_to_tungstenite(args[1].inner())?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let clients = guard.clients.lock().unwrap();
        let client = clients.get(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket client: {id}"
        )))?;
        client
            .tx
            .send(msg)
            .map_err(|_| Error::PrimitiveError("websocket client is closed".to_string()))?;

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
        let clients = guard.clients.lock().unwrap();
        let client = clients.get(&id).ok_or(Error::PrimitiveError(format!(
            "unknown websocket client: {id}"
        )))?;
        let value = client
            .rx
            .lock()
            .unwrap()
            .try_recv()
            .unwrap_or_else(|_| NonFunctionValue::new(JsonValue::Null));
        Ok(value)
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
        let mut clients = guard.clients.lock().unwrap();
        if let Some(client) = clients.remove(&id) {
            let _ = client.tx.send(TungsteniteMessage::Close(None));
        }

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
                    _incoming: Mutex::new(mpsc::channel().1),
                    _shutdown: Mutex::new(None),
                    _handle: Mutex::new(None),
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
        let handler = FunctionValue::from_json(args[2].inner()).ok_or(Error::TypeMismatch {
            expected: "function",
            got: "non-function",
        })?;
        let bind_addr = socket_addr(addr)?;

        let (std_tx, std_rx) = mpsc::channel::<NonFunctionValue>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let (local_addr_tx, local_addr_rx) = mpsc::channel::<SocketAddr>();

        let (eval, next_conn_id) = {
            let inner = inner();
            let guard = inner.lock().unwrap();
            let mut servers = guard.servers.lock().unwrap();
            let server = servers.get_mut(&id).ok_or(Error::PrimitiveError(format!(
                "unknown websocket server: {id}"
            )))?;
            server.handler = Some(handler);
            *server._incoming.get_mut().unwrap() = std_rx;
            let next_conn_id = Arc::new(AtomicUsize::new(1));
            (guard.eval.clone(), next_conn_id)
        };

        let handler = {
            let inner = inner();
            let guard = inner.lock().unwrap();
            let mut servers = guard.servers.lock().unwrap();
            let server = servers.get_mut(&id).unwrap();
            server.handler.clone()
        };

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio server runtime");
            rt.block_on(run_server(
                bind_addr,
                std_tx,
                shutdown_rx,
                local_addr_tx,
                handler,
                eval,
                next_conn_id,
            ));
        });

        let local_addr = local_addr_rx.recv().map_err(|_| {
            Error::PrimitiveError("websocket server failed to bind".to_string())
        })?;

        {
            let inner = inner();
            let guard = inner.lock().unwrap();
            let mut servers = guard.servers.lock().unwrap();
            if let Some(server) = servers.get_mut(&id) {
                server.addr = local_addr.to_string();
                *server._shutdown.get_mut().unwrap() = Some(shutdown_tx);
                *server._handle.get_mut().unwrap() = Some(handle);
            }
        }

        Ok(NonFunctionValue::new(JsonValue::String(format!(
            "ws://{}",
            local_addr
        ))))
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
        let mut servers = guard.servers.lock().unwrap();
        if let Some(server) = servers.remove(&id) {
            if let Some(tx) = server._shutdown.into_inner().unwrap() {
                let _ = tx.send(());
            }
            if let Some(handle) = server._handle.into_inner().unwrap() {
                let _ = handle.join();
            }
        }

        Ok(NonFunctionValue::new(JsonValue::Null))
    }
}

async fn run_server(
    bind_addr: SocketAddr,
    std_tx: mpsc::Sender<NonFunctionValue>,
    mut shutdown_rx: oneshot::Receiver<()>,
    local_addr_tx: mpsc::Sender<SocketAddr>,
    handler: Option<FunctionValue>,
    eval: Arc<CanonicalEval<EvalHost>>,
    next_conn_id: Arc<AtomicUsize>,
) {
    let listener = match TcpListener::bind(bind_addr).await {
        Ok(l) => l,
        Err(_) => return,
    };

    let local_addr = match listener.local_addr() {
        Ok(a) => a,
        Err(_) => return,
    };
    let _ = local_addr_tx.send(local_addr);

    let mut join_set = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => break,
            result = listener.accept() => match result {
                Ok((stream, _)) => {
                    let std_tx = std_tx.clone();
                    let handler = handler.clone();
                    let eval = eval.clone();
                    let next_conn_id = next_conn_id.clone();
                    join_set.spawn(async move {
                        handle_connection(stream, std_tx, handler, eval, next_conn_id).await;
                    });
                }
                Err(_) => break,
            }
        }
    }

    while let Some(res) = join_set.join_next().await {
        let _ = res;
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    std_tx: mpsc::Sender<NonFunctionValue>,
    handler: Option<FunctionValue>,
    eval: Arc<CanonicalEval<EvalHost>>,
    next_conn_id: Arc<AtomicUsize>,
) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
        return;
    };
    let (mut write, mut read) = ws.split();

    while let Some(Ok(msg)) = read.next().await {
        let value = NonFunctionValue::new(tungstenite_to_json(msg));
        if std_tx.send(value.clone()).is_err() {
            break;
        }

        if let Some(fv) = &handler {
            let conn_id = next_conn_id.fetch_add(1, Ordering::Relaxed);
            let conn_value = NonFunctionValue::new(serde_json::json!({
                "connection_id": conn_id,
            }));

            let mut vars: Vec<(String, NonFunctionValue)> = fv
                .params
                .iter()
                .map(|(name, _)| (name.clone(), NonFunctionValue::new(JsonValue::Null)))
                .collect();
            if !vars.is_empty() {
                vars[0].1 = conn_value;
            }
            if vars.len() > 1 {
                vars[1].1 = value;
            }

            match eval.eval_expr(&fv.body, &vars) {
                Ok(result) => match json_to_tungstenite(result.inner()) {
                    Ok(reply) => {
                        if write.send(reply).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => continue,
                },
                Err(_) => continue,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_message_roundtrip() {
        let value = serde_json::json!({"text": "hello"});
        let msg = json_to_tungstenite(&value).unwrap();
        let back = tungstenite_to_json(msg);
        assert_eq!(back, value);
    }

    #[test]
    fn binary_message_roundtrip() {
        let value = serde_json::json!({"binary": BASE64_STANDARD.encode(b"\x00\xff")});
        let msg = json_to_tungstenite(&value).unwrap();
        let back = tungstenite_to_json(msg);
        assert_eq!(back, value);
    }

    #[test]
    fn close_message_roundtrip() {
        let value = serde_json::json!({"close": {"code": 1000, "reason": "bye"}});
        let msg = json_to_tungstenite(&value).unwrap();
        let back = tungstenite_to_json(msg);
        assert_eq!(back, value);
    }

    #[test]
    fn socket_addr_accepts_literal_and_whatwg() {
        let a = socket_addr("127.0.0.1:0").unwrap();
        assert_eq!(a.ip().to_string(), "127.0.0.1");

        let b = socket_addr("ws://127.0.0.1:12345").unwrap();
        assert_eq!(b.port(), 12345);

        let c = socket_addr("127.0.0.1:12345").unwrap();
        assert_eq!(c.port(), 12345);
    }
}
