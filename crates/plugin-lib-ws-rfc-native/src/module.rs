//! The `ws_rfc_native` handl runtime module backed by `tokio-tungstenite`.
//!
//! Provides real TCP/TLS WebSocket clients and servers plus the frame and
//! message helpers from `handl-lib-ws-rfc`.

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use futures_util::{SinkExt, StreamExt};
use handl_plugin_lib_ws_rfc::{FrameDecode, FrameEncode, MessageFromFrame, MessageToFrame};
use handl_platform::{EvalContext, EvalError, Function, Module};
use crate::function_value::FunctionValue;
use serde_json::Value;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
#[cfg(feature = "server")]
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use tokio::io::{AsyncRead, AsyncWrite};
#[cfg(feature = "server")]
use tokio::net::TcpListener;
#[cfg(feature = "client")]
use tokio::runtime::Runtime;
#[cfg(feature = "server")]
use tokio::sync::Semaphore;
#[cfg(feature = "client")]
use tokio::sync::mpsc as tokio_mpsc;
#[cfg(feature = "server")]
use tokio::sync::oneshot;
#[cfg(feature = "server")]
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

static INNER: OnceLock<Arc<Mutex<Inner>>> = OnceLock::new();

#[cfg(feature = "client")]
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

#[cfg(feature = "client")]
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

#[cfg(feature = "client")]
struct Client {
    url: String,
    tx: tokio_mpsc::UnboundedSender<TungsteniteMessage>,
    rx: Arc<Mutex<mpsc::Receiver<Value>>>,
}

#[cfg(feature = "server")]
struct Server {
    addr: String,
    handler: Option<FunctionValue>,
    tls: Option<Arc<tokio_rustls::rustls::ServerConfig>>,
    max_connections: usize,
    _incoming: Mutex<mpsc::Receiver<Value>>,
    _shutdown: Mutex<Option<oneshot::Sender<()>>>,
    _handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct Inner {
    next_id: AtomicUsize,
    #[cfg(feature = "client")]
    clients: Mutex<HashMap<usize, Client>>,
    #[cfg(feature = "server")]
    servers: Mutex<HashMap<usize, Server>>,
}

impl Inner {
    fn new() -> Self {
        Self {
            next_id: AtomicUsize::new(1),
            #[cfg(feature = "client")]
            clients: Mutex::new(HashMap::new()),
            #[cfg(feature = "server")]
            servers: Mutex::new(HashMap::new()),
        }
    }

    fn next_id(&self) -> usize {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

/// The `ws_rfc_native` runtime module with a real `tokio-tungstenite`
/// client/server backend.
#[derive(Clone, Copy)]
pub struct WsRfcNativeModule;

impl Module for WsRfcNativeModule {
    fn name(&self) -> &str {
        "ws_rfc_native"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        #[cfg(feature = "client")]
        let client: Option<&dyn Function> = match name {
            "client" => Some(&ClientNew),
            "connect" => Some(&ClientConnect),
            "send" => Some(&ClientSend),
            "recv" => Some(&ClientRecv),
            "close" => Some(&ClientClose),
            _ => None,
        };
        #[cfg(feature = "server")]
        let server: Option<&dyn Function> = match name {
            "server" => Some(&ServerNew),
            "listen" => Some(&ServerListen),
            "stop" => Some(&ServerStop),
            _ => None,
        };

        #[cfg(feature = "client")]
        if client.is_some() {
            return client;
        }
        #[cfg(feature = "server")]
        if server.is_some() {
            return server;
        }

        match name {
            "frame.encode" => Some(&FrameEncode),
            "frame.decode" => Some(&FrameDecode),
            "message.from_frame" => Some(&MessageFromFrame),
            "message.to_frame" => Some(&MessageToFrame),
            _ => None,
        }
    }
}

fn require_string_at(args: &[Value], idx: usize) -> Result<&str, EvalError> {
    let arg = args.get(idx).ok_or(EvalError::Other(format!("expected idx + 1 arguments, got {}", args.len())))?;
    arg.as_str().ok_or(EvalError::TypeMismatch(format!("expected string, got non-string")))
}

fn require_value_at(args: &[Value], idx: usize) -> Result<&Value, EvalError> {
    args.get(idx).ok_or(EvalError::Other(format!("expected idx + 1 arguments, got {}", args.len())))
}

fn parse_id(value: &Value, key: &str) -> Result<usize, EvalError> {
    if let Some(n) = value.as_f64() {
        return Ok(n as usize);
    }
    let obj = value.as_object().ok_or(EvalError::TypeMismatch(format!("expected number or object, got non-number non-object")))?;
    let id = obj
        .get(key)
        .ok_or_else(|| EvalError::Other(format!("missing {key}")))?;
    let n = id.as_f64().ok_or(EvalError::TypeMismatch(format!("expected number, got non-number")))?;
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
        TungsteniteMessage::Frame(f) => serde_json::json!({
            "frame": {
                "opcode": format!("{:?}", f.header().opcode),
                "payload": BASE64_STANDARD.encode(f.payload()),
                "fin": f.header().is_final,
            }
        }),
    }
}

fn json_to_tungstenite(value: &JsonValue) -> Result<TungsteniteMessage, EvalError> {
    if let Some(text) = value.as_str() {
        return Ok(TungsteniteMessage::Text(text.to_string()));
    }
    let obj = value.as_object().ok_or(EvalError::TypeMismatch(format!("expected string or object, got non-string non-object")))?;
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
        let close_obj = close.as_object().ok_or(EvalError::TypeMismatch(format!("expected object, got non-object")))?;
        let code = close_obj
            .get("code")
            .and_then(|v| v.as_u64())
            .ok_or(EvalError::TypeMismatch(format!("expected u16, got non-number")))?;
        let code = u16::try_from(code).map_err(|_| EvalError::TypeMismatch(format!("expected u16, got out of range")))?;
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
        "websocket message must be {text}, {binary}, {ping}, {pong}, or {close}".to_string(),
    ))
}

#[cfg(feature = "server")]
fn socket_addr(addr: &str) -> Result<SocketAddr, EvalError> {
    if let Ok(sa) = addr.parse::<SocketAddr>() {
        return Ok(sa);
    }
    let url =
        url::Url::parse(addr).map_err(|e| EvalError::Other(format!("url parse: {e}")))?;
    let host = url
        .host_str()
        .ok_or(EvalError::Other("missing host".to_string()))?;
    let port = url
        .port()
        .or_else(|| match url.scheme() {
            "ws" => Some(80),
            "wss" => Some(443),
            _ => None,
        })
        .ok_or(EvalError::Other("missing port".to_string()))?;
    let ip = (host, port)
        .to_socket_addrs()
        .map_err(|e| EvalError::Other(format!("dns lookup: {e}")))?
        .next()
        .ok_or(EvalError::Other(
            "could not resolve address".to_string(),
        ))?;
    Ok(ip)
}

#[cfg(feature = "server")]
fn build_tls_config(
    cert_pem: &str,
    key_pem: &str,
) -> Result<Arc<tokio_rustls::rustls::ServerConfig>, EvalError> {
    use tokio_rustls::rustls::pki_types::CertificateDer;

    let mut cert_reader = cert_pem.as_bytes();
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| EvalError::Other(format!("tls cert parse: {e}")))?;

    let mut key_reader = key_pem.as_bytes();
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| EvalError::Other(format!("tls key parse: {e}")))?
        .ok_or_else(|| EvalError::Other("no private key found".to_string()))?;

    let config = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| EvalError::Other(format!("tls config build: {e}")))?;

    Ok(Arc::new(config))
}

#[cfg(feature = "client")]
#[derive(Clone, Copy)]
struct ClientNew;

#[cfg(feature = "client")]
impl Function for ClientNew {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::Other(format!("expected 0 arguments, got {}", args.len())));
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
                    rx: Arc::new(Mutex::new(mpsc::channel().1)),
                },
            );
            id
        };

        Ok(serde_json::json!({
            "type": "websocket_client",
            "client_id": id,
        }))
    }
}

#[cfg(feature = "client")]
#[derive(Clone, Copy)]
struct ClientConnect;

#[cfg(feature = "client")]
impl Function for ClientConnect {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::Other(format!("expected 2 arguments, got {}", args.len())));
        }

        let id = parse_id(&args[0], "client_id")?;
        let url = require_string_at(args, 1)?;

        let (std_tx, std_rx) = mpsc::channel::<Value>();
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
                            let value = serde_json::json!(tungstenite_to_json(msg));
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
        let client = clients.get_mut(&id).ok_or(EvalError::Other(format!(
            "unknown websocket client: {id}"
        )))?;
        client.url = url_string;
        client.tx = cmd_tx;
        client.rx = Arc::new(Mutex::new(std_rx));

        Ok(Value::Null)
    }
}

#[cfg(feature = "client")]
#[derive(Clone, Copy)]
struct ClientSend;

#[cfg(feature = "client")]
impl Function for ClientSend {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::Other(format!("expected 2 arguments, got {}", args.len())));
        }

        let id = parse_id(&args[0], "client_id")?;
        let msg = json_to_tungstenite(&args[1])?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let clients = guard.clients.lock().unwrap();
        let client = clients.get(&id).ok_or(EvalError::Other(format!(
            "unknown websocket client: {id}"
        )))?;
        client
            .tx
            .send(msg)
            .map_err(|_| EvalError::Other("websocket client is closed".to_string()))?;

        Ok(Value::Null)
    }
}

#[cfg(feature = "client")]
#[derive(Clone, Copy)]
struct ClientRecv;

#[cfg(feature = "client")]
impl Function for ClientRecv {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?, "client_id")?;
        let timeout = args.get(1).and_then(|v| v.as_f64());

        let rx = {
            let inner = inner();
            let guard = inner.lock().unwrap();
            let clients = guard.clients.lock().unwrap();
            let client = clients.get(&id).ok_or(EvalError::Other(format!(
                "unknown websocket client: {id}"
            )))?;
            client.rx.clone()
        };

        let rx = rx.lock().unwrap();
        let value = match timeout {
            None => rx
                .recv()
                .ok()
                .unwrap_or_else(|| Value::Null),
            Some(t) if t <= 0.0 => rx
                .try_recv()
                .unwrap_or_else(|_| Value::Null),
            Some(t) => match rx.recv_timeout(std::time::Duration::from_secs_f64(t)) {
                Ok(v) => v,
                Err(_) => Value::Null,
            },
        };
        Ok(value)
    }
}

#[cfg(feature = "client")]
#[derive(Clone, Copy)]
struct ClientClose;

#[cfg(feature = "client")]
impl Function for ClientClose {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?, "client_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut clients = guard.clients.lock().unwrap();
        if let Some(client) = clients.remove(&id) {
            let _ = client.tx.send(TungsteniteMessage::Close(None));
        }

        Ok(Value::Null)
    }
}

#[cfg(feature = "server")]
#[derive(Clone, Copy)]
struct ServerNew;

#[cfg(feature = "server")]
impl Function for ServerNew {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::Other(format!("expected 0 arguments, got {}", args.len())));
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
                    tls: None,
                    max_connections: 1024,
                    _incoming: Mutex::new(mpsc::channel().1),
                    _shutdown: Mutex::new(None),
                    _handle: Mutex::new(None),
                },
            );
            id
        };

        Ok(serde_json::json!({
            "type": "websocket_server",
            "server_id": id,
        }))
    }
}

#[cfg(feature = "server")]
#[derive(Clone, Copy)]
struct ServerListen;

#[cfg(feature = "server")]
impl Function for ServerListen {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.len() < 3 {
            return Err(EvalError::Other(format!("expected 3 arguments, got {}", args.len())));
        }

        let id = parse_id(&args[0], "server_id")?;
        let addr = require_string_at(args, 1)?;
        let handler = FunctionValue::from_json(&args[2]).ok_or(EvalError::TypeMismatch(format!("expected function, got non-function")))?;
        let is_wss = addr.starts_with("wss://");
        let bind_addr = socket_addr(addr)?;

        let (tls, max_connections) = if let Some(arg) = args.get(3) {
            let obj = arg.as_object().ok_or(EvalError::TypeMismatch(format!("expected object, got non-object")))?;
            let cert = obj.get("cert").and_then(|v| v.as_str());
            let key = obj.get("key").and_then(|v| v.as_str());
            let tls = match (cert, key, is_wss) {
                (Some(c), Some(k), true) => Some(build_tls_config(c, k)?),
                (Some(_), _, false) | (_, Some(_), false) => {
                    return Err(EvalError::Other(
                        "tls cert/key require wss:// address".to_string(),
                    ));
                }
                _ if is_wss => {
                    return Err(EvalError::Other(
                        "wss:// requires cert and key in config".to_string(),
                    ));
                }
                _ => None,
            };
            let max = obj
                .get("max_connections")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(1024);
            (tls, max)
        } else if is_wss {
            return Err(EvalError::Other(
                "wss:// requires a config object with cert and key".to_string(),
            ));
        } else {
            (None, 1024)
        };

        let (std_tx, std_rx) = mpsc::channel::<Value>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let (local_addr_tx, local_addr_rx) = mpsc::channel::<SocketAddr>();

        let next_conn_id = {
            let inner = inner();
            let guard = inner.lock().unwrap();
            let mut servers = guard.servers.lock().unwrap();
            let server = servers.get_mut(&id).ok_or(EvalError::Other(format!(
                "unknown websocket server: {id}"
            )))?;
            server.handler = Some(handler);
            server.tls = tls.clone();
            server.max_connections = max_connections;
            *server._incoming.get_mut().unwrap() = std_rx;
            let next_conn_id = Arc::new(AtomicUsize::new(1));
            (next_conn_id)
        };

        let (handler, tls, max_connections) = {
            let inner = inner();
            let guard = inner.lock().unwrap();
            let mut servers = guard.servers.lock().unwrap();
            let server = servers.get_mut(&id).unwrap();
            (
                server.handler.clone(),
                server.tls.clone(),
                server.max_connections,
            )
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
                next_conn_id,
                tls,
                max_connections,
            ));
        });

        let local_addr = local_addr_rx
            .recv()
            .map_err(|_| EvalError::Other("websocket server failed to bind".to_string()))?;

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

        let scheme = if is_wss { "wss" } else { "ws" };
        Ok(serde_json::json!(JsonValue::String(format!(
            "{}://{}",
            scheme, local_addr
        ))))
    }
}

#[cfg(feature = "server")]
#[derive(Clone, Copy)]
struct ServerStop;

#[cfg(feature = "server")]
impl Function for ServerStop {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        let id = parse_id(require_value_at(args, 0)?, "server_id")?;

        let inner = inner();
        let guard = inner.lock().unwrap();
        let mut servers = guard.servers.lock().unwrap();
        if let Some(server) = servers.remove(&id) {
            if let Some(tx) = server._shutdown.lock().unwrap().take() {
                let _ = tx.send(());
            }
            if let Some(handle) = server._handle.lock().unwrap().take() {
                let _ = handle.join();
            }
        }

        Ok(Value::Null)
    }
}

#[cfg(feature = "server")]
async fn run_server(
    bind_addr: SocketAddr,
    std_tx: mpsc::Sender<Value>,
    mut shutdown_rx: oneshot::Receiver<()>,
    local_addr_tx: mpsc::Sender<SocketAddr>,
    handler: Option<FunctionValue>,
    next_conn_id: Arc<AtomicUsize>,
    tls: Option<Arc<tokio_rustls::rustls::ServerConfig>>,
    max_connections: usize,
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

    let sem = Arc::new(Semaphore::new(max_connections));
    let mut join_set = tokio::task::JoinSet::new();
    let acceptor = tls.map(TlsAcceptor::from);

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => break,
            result = listener.accept() => match result {
                Ok((stream, _)) => {
                    let permit = match Arc::clone(&sem).acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    let std_tx = std_tx.clone();
                    let handler = handler.clone();
                    
                    let next_conn_id = next_conn_id.clone();
                    if let Some(acceptor) = acceptor.clone() {
                        match acceptor.accept(stream).await {
                            Ok(tls_stream) => {
                                join_set.spawn(async move {
                                    let _permit = permit;
                                    handle_connection(tls_stream, std_tx, handler, next_conn_id).await;
                                });
                            }
                            Err(_) => {
                                drop(permit);
                                continue;
                            }
                        }
                    } else {
                        join_set.spawn(async move {
                            let _permit = permit;
                            handle_connection(stream, std_tx, handler, next_conn_id).await;
                        });
                    }
                }
                Err(_) => break,
            }
        }
    }

    while let Some(res) = join_set.join_next().await {
        let _ = res;
    }
}

#[cfg(feature = "server")]
async fn handle_connection<S>(
    stream: S,
    std_tx: mpsc::Sender<Value>,
    handler: Option<FunctionValue>,
    next_conn_id: Arc<AtomicUsize>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
        return;
    };
    let (mut write, mut read) = ws.split();

    while let Some(Ok(msg)) = read.next().await {
        let value = serde_json::json!(tungstenite_to_json(msg));
        if std_tx.send(value.clone()).is_err() {
            break;
        }

        if let Some(_fv) = &handler {
            // TODO(ADR 0040 migration): The old system used CanonicalEval<EvalHost>
            // to evaluate function value handlers. The new ExprEvaluator trait is
            // not Send + Sync, so it cannot be used inside tokio::spawn. For now,
            // function value handlers are not supported in the native server.
            // A proper migration would use a channel-based approach to send
            // evaluation requests back to a single-threaded eval loop.
            continue;
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

    #[cfg(feature = "server")]
    #[test]
    fn socket_addr_accepts_literal_and_whatwg() {
        let a = socket_addr("127.0.0.1:0").unwrap();
        assert_eq!(a.ip().to_string(), "127.0.0.1");

        let b = socket_addr("ws://127.0.0.1:12345").unwrap();
        assert_eq!(b.port(), 12345);

        let c = socket_addr("127.0.0.1:12345").unwrap();
        assert_eq!(c.port(), 12345);
    }

    #[cfg(all(feature = "client", feature = "server"))]
    #[test]
    #[ignore = "network/tls integration"]
    fn client_server_echo_roundtrip() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let server_id = m.function("server").unwrap().call(&[], &mut ctx).unwrap();

        let handler = serde_json::json!({
            "params": [
                {"name": "conn", "schema": null},
                {"name": "msg", "schema": null},
            ],
            "body": "msg",
        });

        let url = m
            .function("listen")
            .unwrap()
            .call(
                &[
                    server_id.clone(),
                    serde_json::json!("127.0.0.1:0"),
                    handler,
                ],
                &mut ctx,
            )
            .unwrap();

        let client = m.function("client").unwrap().call(&[], &mut ctx).unwrap();

        m.function("connect")
            .unwrap()
            .call(&[client.clone(), url], &mut ctx)
            .unwrap();

        m.function("send")
            .unwrap()
            .call(
                &[
                    client.clone(),
                    serde_json::json!("hello"),
                ],
                &mut ctx,
            )
            .unwrap();

        let msg = m
            .function("recv")
            .unwrap()
            .call(&[client.clone()], &mut ctx)
            .unwrap();

        assert_eq!(msg.get("text"), Some(&serde_json::json!("hello")));

        m.function("close")
            .unwrap()
            .call(&[client], &mut ctx)
            .unwrap();

        m.function("stop")
            .unwrap()
            .call(&[server_id], &mut ctx)
            .unwrap();
    }

    #[cfg(feature = "server")]
    #[test]
    fn socket_addr_default_ports() {
        let ws = socket_addr("ws://127.0.0.1").unwrap();
        assert_eq!(ws.port(), 80);

        let wss = socket_addr("wss://127.0.0.1").unwrap();
        assert_eq!(wss.port(), 443);
    }

    #[cfg(feature = "server")]
    #[test]
    fn socket_addr_rejects_garbage() {
        assert!(socket_addr("not-an-address").is_err());
    }

    #[cfg(feature = "client")]
    #[test]
    fn recv_unconnected_client_returns_null() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let client = m.function("client").unwrap().call(&[], &mut ctx).unwrap();

        let msg = m
            .function("recv")
            .unwrap()
            .call(&[client], &mut ctx)
            .unwrap();

        assert!(msg.is_null());
    }

    #[cfg(feature = "server")]
    #[test]
    fn listen_to_invalid_addr_fails() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let server_id = m.function("server").unwrap().call(&[], &mut ctx).unwrap();

        let handler = serde_json::json!({
            "params": [{"name": "conn", "schema": null}, {"name": "msg", "schema": null}],
            "body": "msg",
        });

        let result = m.function("listen").unwrap().call(
            &[
                server_id,
                serde_json::json!("not-an-address"),
                handler,
            ],
            &mut ctx,
        );

        assert!(result.is_err());
    }

    #[cfg(all(feature = "client", feature = "server"))]
    #[test]
    #[ignore = "network/tls integration"]
    fn client_server_binary_echo_roundtrip() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let server_id = m.function("server").unwrap().call(&[], &mut ctx).unwrap();
        let handler = serde_json::json!({
            "params": [{"name": "conn", "schema": null}, {"name": "msg", "schema": null}],
            "body": "msg",
        });

        let url = m
            .function("listen")
            .unwrap()
            .call(
                &[
                    server_id.clone(),
                    serde_json::json!("127.0.0.1:0"),
                    handler,
                ],
                &mut ctx,
            )
            .unwrap();

        let client = m.function("client").unwrap().call(&[], &mut ctx).unwrap();
        m.function("connect")
            .unwrap()
            .call(&[client.clone(), url], &mut ctx)
            .unwrap();

        let payload = BASE64_STANDARD.encode(b"\x00\xff");
        m.function("send")
            .unwrap()
            .call(
                &[
                    client.clone(),
                    serde_json::json!({"binary": payload}),
                ],
                &mut ctx,
            )
            .unwrap();

        let msg = m
            .function("recv")
            .unwrap()
            .call(&[client.clone()], &mut ctx)
            .unwrap();
        assert_eq!(msg.get("binary"), Some(&serde_json::json!(payload)));

        m.function("close")
            .unwrap()
            .call(&[client], &mut ctx)
            .unwrap();
        m.function("stop")
            .unwrap()
            .call(&[server_id], &mut ctx)
            .unwrap();
    }

    #[cfg(feature = "client")]
    #[test]
    fn connect_to_refused_port_fails() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let client = m.function("client").unwrap().call(&[], &mut ctx).unwrap();
        let result = m.function("connect").unwrap().call(
            &[
                client,
                serde_json::json!("ws://127.0.0.1:1"),
            ],
            &mut ctx,
        );

        assert!(result.is_err());
    }

    #[cfg(feature = "client")]
    #[test]
    fn recv_non_blocking_returns_null_when_empty() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let client = m.function("client").unwrap().call(&[], &mut ctx).unwrap();
        let msg = m
            .function("recv")
            .unwrap()
            .call(
                &[client, serde_json::json!(0)],
                &mut ctx,
            )
            .unwrap();

        assert!(msg.is_null());
    }

    #[cfg(all(feature = "client", feature = "server"))]
    #[test]
    #[ignore = "network/tls integration"]
    fn wss_server_echo_roundtrip() {
        use handl_platform::EvalContext;
        use tokio::runtime::Runtime;
        use tokio_rustls::rustls::pki_types::ServerName;
        use tokio_rustls::rustls::{ClientConfig, RootCertStore};

        let cert = rcgen::generate_simple_self_signed(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
        ])
        .unwrap();
        let cert_pem = cert.cert.pem();
        let key_pem = cert.key_pair.serialize_pem();

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let server_id = m.function("server").unwrap().call(&[], &mut ctx).unwrap();
        let handler = serde_json::json!({
            "params": [{"name": "conn", "schema": null}, {"name": "msg", "schema": null}],
            "body": "msg",
        });

        let url = m
            .function("listen")
            .unwrap()
            .call(
                &[
                    server_id.clone(),
                    serde_json::json!("wss://127.0.0.1:0"),
                    handler,
                    serde_json::json!({
                        "cert": cert_pem,
                        "key": key_pem,
                    }),
                ],
                &mut ctx,
            )
            .unwrap()
            
            .as_str()
            .unwrap()
            .to_string();

        let reply = Runtime::new().unwrap().block_on(async {
            let mut roots = RootCertStore::empty();
            let mut reader = cert_pem.as_bytes();
            for cert in rustls_pemfile::certs(&mut reader)
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
            {
                roots.add(cert).unwrap();
            }
            let client_config = ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));

            let bind = url
                .strip_prefix("wss://")
                .unwrap()
                .parse::<std::net::SocketAddr>()
                .unwrap();
            let tcp = tokio::net::TcpStream::connect(bind).await.unwrap();
            let server_name = ServerName::IpAddress(std::net::Ipv4Addr::new(127, 0, 0, 1).into());
            let tls = connector.connect(server_name, tcp).await.unwrap();

            let (mut ws, _) = tokio_tungstenite::client_async(url.as_str(), tls)
                .await
                .unwrap();

            ws.send(TungsteniteMessage::Text("hello".to_string()))
                .await
                .unwrap();
            let reply = ws.next().await.unwrap().unwrap();
            let _ = ws.close(None).await;
            reply
        });

        assert_eq!(
            tungstenite_to_json(reply),
            serde_json::json!({"text": "hello"})
        );

        m.function("stop")
            .unwrap()
            .call(&[server_id], &mut ctx)
            .unwrap();
    }

    #[cfg(feature = "server")]
    #[test]
    fn listen_accepts_max_connections_config() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let server_id = m.function("server").unwrap().call(&[], &mut ctx).unwrap();
        let handler = serde_json::json!({
            "params": [{"name": "conn", "schema": null}, {"name": "msg", "schema": null}],
            "body": "msg",
        });

        let url = m
            .function("listen")
            .unwrap()
            .call(
                &[
                    server_id.clone(),
                    serde_json::json!("127.0.0.1:0"),
                    handler,
                    serde_json::json!({
                        "max_connections": 2,
                    }),
                ],
                &mut ctx,
            )
            .unwrap();

        assert!(url.as_str().unwrap().starts_with("ws://"));

        m.function("stop")
            .unwrap()
            .call(&[server_id], &mut ctx)
            .unwrap();
    }

    #[cfg(feature = "client")]
    #[test]
    fn connect_to_invalid_url_fails() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let client = m.function("client").unwrap().call(&[], &mut ctx).unwrap();
        let result = m.function("connect").unwrap().call(
            &[
                client,
                serde_json::json!("not-a-url"),
            ],
            &mut ctx,
        );

        assert!(result.is_err());
    }

    #[cfg(feature = "client")]
    #[test]
    fn send_to_unknown_client_fails() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let result = m.function("send").unwrap().call(
            &[
                serde_json::json!({"client_id": 99999}),
                serde_json::json!("hello"),
            ],
            &mut ctx,
        );

        assert!(result.is_err());
    }

    #[cfg(feature = "client")]
    #[test]
    fn recv_from_unknown_client_fails() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let result = m.function("recv").unwrap().call(
            &[serde_json::json!({"client_id": 99999})],
            &mut ctx,
        );

        assert!(result.is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn socket_addr_http_scheme_rejected() {
        assert!(socket_addr("http://127.0.0.1").is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn build_tls_config_rejects_invalid_pem() {
        assert!(build_tls_config("not a cert", "not a key").is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn listen_wss_requires_cert_config() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let server_id = m.function("server").unwrap().call(&[], &mut ctx).unwrap();
        let handler = serde_json::json!({
            "params": [{"name": "conn", "schema": null}, {"name": "msg", "schema": null}],
            "body": "msg",
        });

        let result = m.function("listen").unwrap().call(
            &[
                server_id,
                serde_json::json!("wss://127.0.0.1:0"),
                handler,
            ],
            &mut ctx,
        );

        assert!(result.is_err());
    }

    #[cfg(feature = "client")]
    #[test]
    fn client_with_arguments_errors() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let result = m
            .function("client")
            .unwrap()
            .call(&[serde_json::json!(null)], &mut ctx);
        assert!(result.is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn server_with_arguments_errors() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let result = m
            .function("server")
            .unwrap()
            .call(&[serde_json::json!(null)], &mut ctx);
        assert!(result.is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn listen_rejects_non_object_config() {
        use handl_platform::EvalContext;

        let mut ctx = EvalContext::new();
        let m = WsRfcNativeModule;

        let server_id = m.function("server").unwrap().call(&[], &mut ctx).unwrap();
        let handler = serde_json::json!({
            "params": [{"name": "conn", "schema": null}, {"name": "msg", "schema": null}],
            "body": "msg",
        });

        let result = m.function("listen").unwrap().call(
            &[
                server_id,
                serde_json::json!("ws://127.0.0.1:0"),
                handler,
                serde_json::json!("not-an-object"),
            ],
            &mut ctx,
        );
        assert!(result.is_err());
    }
}
