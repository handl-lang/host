use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::thread;

use handl_platform::EvalError;
use mio::net::TcpStream as MioTcpStream;
use mio::{Events, Interest, Poll, Token, Waker};
use parking_lot::Mutex;
use serde_json::Value;

/// Backend trait for blocking async I/O primitives.
pub trait IoRuntime {
    /// Open a TCP connection to `addr` and return a socket handle.
    fn tcp_connect(&self, addr: Value) -> Result<Value, EvalError>;

    /// Send `data` over `socket`.
    fn tcp_send(&self, socket: Value, data: Value) -> Result<Value, EvalError>;

    /// Receive up to 4096 bytes from `socket`.
    fn tcp_recv(&self, socket: Value) -> Result<Value, EvalError>;

    /// Close `socket`.
    fn tcp_close(&self, socket: Value) -> Result<Value, EvalError>;
}

fn socket_id(socket: &Value) -> Result<u64, EvalError> {
    socket
        .as_object()
        .and_then(|o| o.get("id"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| EvalError::Other("expected a tcp_socket value".to_string()))
}

fn tcp_socket_value(id: u64) -> Value {
    serde_json::json!({"type": "tcp_socket", "id": id})
}

/// Blocking `std::net::TcpStream` I/O backend.
///
/// Stores open streams in a mutex-protected map indexed by an auto-incrementing id.
pub struct StdNetIoRuntime {
    streams: Arc<Mutex<HashMap<u64, TcpStream>>>,
    next_id: AtomicU64,
}

impl StdNetIoRuntime {
    /// Creates a new `StdNetIoRuntime`.
    pub fn new() -> Self {
        Self {
            streams: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(0),
        }
    }
}

impl Default for StdNetIoRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl IoRuntime for StdNetIoRuntime {
    fn tcp_connect(&self, addr: Value) -> Result<Value, EvalError> {
        let addr = addr.as_str().ok_or_else(|| {
            EvalError::Other("tcp_connect expected a host:port string".to_string())
        })?;

        let stream = TcpStream::connect(addr).map_err(|e| EvalError::Other(e.to_string()))?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.streams.lock().insert(id, stream);

        Ok(tcp_socket_value(id))
    }

    fn tcp_send(&self, socket: Value, data: Value) -> Result<Value, EvalError> {
        let id = socket_id(&socket)?;
        let mut streams = self.streams.lock();
        let stream = streams
            .get_mut(&id)
            .ok_or_else(|| EvalError::Other("tcp_send: unknown socket".to_string()))?;

        let payload = data
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| data.to_string());

        stream
            .write_all(payload.as_bytes())
            .map_err(|e| EvalError::Other(e.to_string()))?;

        Ok(serde_json::json!(null))
    }

    fn tcp_recv(&self, socket: Value) -> Result<Value, EvalError> {
        let id = socket_id(&socket)?;
        let mut streams = self.streams.lock();
        let stream = streams
            .get_mut(&id)
            .ok_or_else(|| EvalError::Other("tcp_recv: unknown socket".to_string()))?;

        let mut buf = [0u8; 4096];
        let n = stream
            .read(&mut buf)
            .map_err(|e| EvalError::Other(e.to_string()))?;

        let text = String::from_utf8_lossy(&buf[..n]).to_string();
        Ok(serde_json::json!(text))
    }

    fn tcp_close(&self, socket: Value) -> Result<Value, EvalError> {
        let id = socket_id(&socket)?;
        let mut streams = self.streams.lock();
        if streams.remove(&id).is_none() {
            return Err(EvalError::Other("tcp_close: unknown socket".to_string()));
        }
        Ok(serde_json::json!(null))
    }
}

const WAKER: Token = Token(0);

enum Command {
    Connect {
        addr: SocketAddr,
        token: Token,
        tx: Sender<Result<u64, String>>,
    },
    Send {
        token: Token,
        data: Vec<u8>,
        tx: Sender<Result<usize, String>>,
    },
    Recv {
        token: Token,
        tx: Sender<Result<String, String>>,
    },
    Close {
        token: Token,
        tx: Sender<Result<(), String>>,
    },
    Shutdown(Sender<()>),
}

struct Sending {
    data: Vec<u8>,
    offset: usize,
    tx: Sender<Result<usize, String>>,
}

struct MioSocketState {
    stream: MioTcpStream,
    connected: bool,
    registered: bool,
    connect_tx: Option<Sender<Result<u64, String>>>,
    send: Option<Sending>,
    recv: Option<Sender<Result<String, String>>>,
}

fn update_interest(registry: &mio::Registry, token: Token, state: &mut MioSocketState) {
    if !state.connected {
        if state.registered {
            let _ = registry.reregister(&mut state.stream, token, Interest::WRITABLE);
        } else {
            if registry
                .register(&mut state.stream, token, Interest::WRITABLE)
                .is_ok()
            {
                state.registered = true;
            }
        }
        return;
    }
    let interest = match (state.send.is_some(), state.recv.is_some()) {
        (true, true) => Interest::READABLE | Interest::WRITABLE,
        (true, false) => Interest::WRITABLE,
        (false, true) => Interest::READABLE,
        (false, false) => {
            if state.registered {
                let _ = registry.deregister(&mut state.stream);
                state.registered = false;
            }
            return;
        }
    };
    if state.registered {
        let _ = registry.reregister(&mut state.stream, token, interest);
    } else if registry
        .register(&mut state.stream, token, interest)
        .is_ok()
    {
        state.registered = true;
    }
}

fn handle_writable(token: Token, state: &mut MioSocketState, registry: &mio::Registry) -> bool {
    if !state.connected {
        match state.stream.take_error() {
            Ok(None) => match state.stream.peer_addr() {
                Ok(_) => {
                    state.connected = true;
                    if let Some(tx) = state.connect_tx.take() {
                        let _ = tx.send(Ok(token.0 as u64));
                    }
                }
                Err(err) if err.kind() == ErrorKind::NotConnected => return false,
                Err(err) => {
                    let e = err.to_string();
                    if let Some(tx) = state.connect_tx.take() {
                        let _ = tx.send(Err(e));
                    }
                    if state.registered {
                        let _ = registry.deregister(&mut state.stream);
                        state.registered = false;
                    }
                    return true;
                }
            },
            Ok(Some(err)) => {
                let e = err.to_string();
                if let Some(tx) = state.connect_tx.take() {
                    let _ = tx.send(Err(e));
                }
                if state.registered {
                    let _ = registry.deregister(&mut state.stream);
                    state.registered = false;
                }
                return true;
            }
            Err(err) => {
                let e = err.to_string();
                if let Some(tx) = state.connect_tx.take() {
                    let _ = tx.send(Err(e));
                }
                if state.registered {
                    let _ = registry.deregister(&mut state.stream);
                    state.registered = false;
                }
                return true;
            }
        }
    }
    if let Some(mut sending) = state.send.take() {
        loop {
            let remaining = &sending.data[sending.offset..];
            if remaining.is_empty() {
                let _ = sending.tx.send(Ok(sending.data.len()));
                break;
            }
            match state.stream.write(remaining) {
                Ok(0) => break,
                Ok(n) => {
                    sending.offset += n;
                    if sending.offset == sending.data.len() {
                        let _ = sending.tx.send(Ok(sending.data.len()));
                        break;
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => {
                    let _ = sending.tx.send(Err(e.to_string()));
                    break;
                }
            }
        }
        if sending.offset < sending.data.len() {
            state.send = Some(sending);
        }
    }
    update_interest(registry, token, state);
    false
}

fn handle_readable(token: Token, state: &mut MioSocketState, registry: &mio::Registry) -> bool {
    if !state.connected {
        return false;
    }
    if let Some(tx) = state.recv.take() {
        let mut buf = [0u8; 4096];
        match state.stream.read(&mut buf) {
            Ok(0) => {
                let _ = tx.send(Ok(String::new()));
            }
            Ok(n) => {
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.send(Ok(text));
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                state.recv = Some(tx);
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {
                state.recv = Some(tx);
            }
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
            }
        }
    }
    update_interest(registry, token, state);
    false
}

/// Non-blocking `mio`-based TCP I/O backend.
///
/// A dedicated event loop thread multiplexes `mio::net::TcpStream`s behind
/// a command queue. The public methods block on one-shot result channels.
pub struct MioIoRuntime {
    tx: Sender<Command>,
    waker: Mutex<Waker>,
    next_token: AtomicUsize,
    #[allow(dead_code)]
    sockets: Arc<Mutex<HashMap<Token, MioSocketState>>>,
}

impl MioIoRuntime {
    /// Creates a new `MioIoRuntime` with a background `mio::Poll` event loop.
    pub fn new() -> Self {
        let (tx, rx) = channel::<Command>();
        let (waker_tx, waker_rx) = channel::<Waker>();
        let sockets: Arc<Mutex<HashMap<Token, MioSocketState>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let sockets2 = sockets.clone();
        let next_token = AtomicUsize::new(1);
        thread::spawn(move || {
            let mut poll = match Poll::new() {
                Ok(p) => p,
                Err(_) => return,
            };
            let waker = match Waker::new(poll.registry(), WAKER) {
                Ok(w) => w,
                Err(_) => return,
            };
            let _ = waker_tx.send(waker);
            let mut events = Events::with_capacity(1024);
            'event_loop: loop {
                match poll.poll(&mut events, None) {
                    Ok(_) => {}
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
                for event in events.iter() {
                    if event.token() == WAKER {
                        continue;
                    }
                    let mut guard = sockets2.lock();
                    let remove = if let Some(state) = guard.get_mut(&event.token()) {
                        if event.is_writable() {
                            let remove = handle_writable(event.token(), state, poll.registry());
                            if remove {
                                true
                            } else if event.is_readable() {
                                handle_readable(event.token(), state, poll.registry())
                            } else {
                                false
                            }
                        } else if event.is_readable() {
                            handle_readable(event.token(), state, poll.registry())
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if remove {
                        guard.remove(&event.token());
                    }
                }
                while let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        Command::Connect { addr, token, tx } => match MioTcpStream::connect(addr) {
                            Ok(stream) => {
                                let state = MioSocketState {
                                    stream,
                                    connected: false,
                                    registered: false,
                                    connect_tx: Some(tx),
                                    send: None,
                                    recv: None,
                                };
                                let mut guard = sockets2.lock();
                                guard.insert(token, state);
                                if let Some(state) = guard.get_mut(&token) {
                                    update_interest(poll.registry(), token, state);
                                }
                            }
                            Err(err) => {
                                let _ = tx.send(Err(err.to_string()));
                            }
                        },
                        Command::Send { token, data, tx } => {
                            let mut guard = sockets2.lock();
                            let mut remove = false;
                            if let Some(state) = guard.get_mut(&token) {
                                if state.send.is_some() {
                                    let _ = tx.send(Err(
                                        "tcp_send: a send is already pending".to_string()
                                    ));
                                } else {
                                    state.send = Some(Sending {
                                        data,
                                        offset: 0,
                                        tx,
                                    });
                                    update_interest(poll.registry(), token, state);
                                    if state.connected {
                                        remove = handle_writable(token, state, poll.registry());
                                    }
                                }
                            } else {
                                let _ = tx.send(Err("tcp_send: unknown socket".to_string()));
                            }
                            if remove {
                                guard.remove(&token);
                            }
                        }
                        Command::Recv { token, tx } => {
                            let mut guard = sockets2.lock();
                            let mut remove = false;
                            if let Some(state) = guard.get_mut(&token) {
                                if state.recv.is_some() {
                                    let _ = tx.send(Err(
                                        "tcp_recv: a recv is already pending".to_string()
                                    ));
                                } else {
                                    state.recv = Some(tx);
                                    update_interest(poll.registry(), token, state);
                                    if state.connected {
                                        remove = handle_readable(token, state, poll.registry());
                                    }
                                }
                            } else {
                                let _ = tx.send(Err("tcp_recv: unknown socket".to_string()));
                            }
                            if remove {
                                guard.remove(&token);
                            }
                        }
                        Command::Close { token, tx } => {
                            let mut guard = sockets2.lock();
                            if let Some(mut state) = guard.remove(&token) {
                                let _ = poll.registry().deregister(&mut state.stream);
                                let _ = tx.send(Ok(()));
                            } else {
                                let _ = tx.send(Err("tcp_close: unknown socket".to_string()));
                            }
                        }
                        Command::Shutdown(ack) => {
                            let _ = ack.send(());
                            break 'event_loop;
                        }
                    }
                }
            }
        });
        let waker = waker_rx.recv().expect("mio waker");
        Self {
            tx,
            waker: Mutex::new(waker),
            next_token,
            sockets,
        }
    }
}

impl Default for MioIoRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl IoRuntime for MioIoRuntime {
    fn tcp_connect(&self, addr: Value) -> Result<Value, EvalError> {
        let addr = addr.as_str().ok_or_else(|| {
            EvalError::Other("tcp_connect expected a host:port string".to_string())
        })?;
        let mut addrs = addr
            .to_socket_addrs()
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let socket_addr = addrs
            .next()
            .ok_or_else(|| EvalError::Other("tcp_connect could not resolve address".to_string()))?;
        let id = self.next_token.fetch_add(1, Ordering::SeqCst) as u64;
        let token = Token(id as usize);
        let (tx, rx) = channel::<Result<u64, String>>();
        self.tx
            .send(Command::Connect {
                addr: socket_addr,
                token,
                tx,
            })
            .map_err(|_| EvalError::Other("io backend disconnected".to_string()))?;
        let _ = self.waker.lock().wake();
        let result = rx
            .recv()
            .map_err(|_| EvalError::Other("io backend disconnected".to_string()))?;
        match result {
            Ok(id) => Ok(tcp_socket_value(id)),
            Err(e) => Err(EvalError::Other(e)),
        }
    }

    fn tcp_send(&self, socket: Value, data: Value) -> Result<Value, EvalError> {
        let id = socket_id(&socket)?;
        let token = Token(id as usize);
        let payload = data
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| data.to_string());
        let (tx, rx) = channel::<Result<usize, String>>();
        self.tx
            .send(Command::Send {
                token,
                data: payload.into_bytes(),
                tx,
            })
            .map_err(|_| EvalError::Other("io backend disconnected".to_string()))?;
        let _ = self.waker.lock().wake();
        match rx.recv() {
            Ok(Ok(_)) => Ok(serde_json::json!(null)),
            Ok(Err(e)) => Err(EvalError::Other(e)),
            Err(_) => Err(EvalError::Other("io backend disconnected".to_string())),
        }
    }

    fn tcp_recv(&self, socket: Value) -> Result<Value, EvalError> {
        let id = socket_id(&socket)?;
        let token = Token(id as usize);
        let (tx, rx) = channel::<Result<String, String>>();
        self.tx
            .send(Command::Recv { token, tx })
            .map_err(|_| EvalError::Other("io backend disconnected".to_string()))?;
        let _ = self.waker.lock().wake();
        let text = rx
            .recv()
            .map_err(|_| EvalError::Other("io backend disconnected".to_string()))?
            .map_err(EvalError::Other)?;
        Ok(serde_json::json!(text))
    }

    fn tcp_close(&self, socket: Value) -> Result<Value, EvalError> {
        let id = socket_id(&socket)?;
        let token = Token(id as usize);
        let (tx, rx) = channel::<Result<(), String>>();
        self.tx
            .send(Command::Close { token, tx })
            .map_err(|_| EvalError::Other("io backend disconnected".to_string()))?;
        let _ = self.waker.lock().wake();
        rx.recv()
            .map_err(|_| EvalError::Other("io backend disconnected".to_string()))?
            .map_err(EvalError::Other)?;
        Ok(serde_json::json!(null))
    }
}

impl Drop for MioIoRuntime {
    fn drop(&mut self) {
        let (tx, rx) = channel::<()>();
        let _ = self.tx.send(Command::Shutdown(tx));
        let _ = self.waker.lock().wake();
        let _ = rx.recv();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn tcp_round_trip() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap();
            stream.write_all(&buf[..n]).unwrap();
        });

        let io = StdNetIoRuntime::new();
        let socket = io.tcp_connect(serde_json::json!(addr)).unwrap();

        io.tcp_send(socket.clone(), serde_json::json!("hello"))
            .unwrap();

        let resp = io.tcp_recv(socket.clone()).unwrap();
        assert_eq!(resp.as_str(), Some("hello"));

        io.tcp_close(socket).unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn tcp_connect_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        drop(listener);

        let io = StdNetIoRuntime::new();
        let res = io.tcp_connect(serde_json::json!(addr));
        assert!(res.is_err(), "expected connection refused");
    }

    #[test]
    fn tcp_send_to_closed_socket_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let _handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap();
            stream.write_all(&buf[..n]).unwrap();
        });

        let io = StdNetIoRuntime::new();
        let socket = io.tcp_connect(serde_json::json!(addr)).unwrap();

        io.tcp_close(socket.clone()).unwrap();
        let res = io.tcp_send(socket, serde_json::json!("hello"));
        assert!(res.is_err(), "expected send on closed socket to error");
    }

    #[test]
    fn tcp_recv_empty_on_no_data() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let _handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            drop(stream);
        });

        let io = StdNetIoRuntime::new();
        let socket = io.tcp_connect(serde_json::json!(addr)).unwrap();

        let resp = io.tcp_recv(socket).unwrap();
        assert_eq!(resp.as_str(), Some(""));
    }

    #[test]
    fn tcp_close_unknown_id_error() {
        let io = StdNetIoRuntime::new();
        let socket = serde_json::json!({ "type": "tcp_socket", "id": 999 });
        let res = io.tcp_close(socket);
        assert!(res.is_err(), "expected close on unknown socket to error");
    }

    #[test]
    fn tcp_send_with_non_string_data_uses_debug() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap();
            stream.write_all(&buf[..n]).unwrap();
        });

        let io = StdNetIoRuntime::new();
        let socket = io.tcp_connect(serde_json::json!(addr)).unwrap();

        io.tcp_send(socket.clone(), serde_json::json!(123)).unwrap();

        let resp = io.tcp_recv(socket).unwrap();
        assert_eq!(resp.as_str(), Some("123"));

        handle.join().unwrap();
    }

    #[test]
    fn tcp_connect_invalid_address_error() {
        let io = StdNetIoRuntime::new();
        let res = io.tcp_connect(serde_json::json!("not-a-host:12345"));
        assert!(res.is_err(), "expected invalid address to error");
    }

    #[test]
    fn mio_tcp_round_trip() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap();
            stream.write_all(&buf[..n]).unwrap();
        });

        let io = MioIoRuntime::new();
        let socket = io.tcp_connect(serde_json::json!(addr)).unwrap();

        io.tcp_send(socket.clone(), serde_json::json!("hello"))
            .unwrap();

        let resp = io.tcp_recv(socket.clone()).unwrap();
        assert_eq!(resp.as_str(), Some("hello"));

        io.tcp_close(socket).unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn mio_tcp_connect_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        drop(listener);

        let io = MioIoRuntime::new();
        let res = io.tcp_connect(serde_json::json!(addr));
        assert!(res.is_err(), "expected connection refused");
    }

    #[test]
    fn mio_tcp_send_multiple_messages() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            loop {
                let n = match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => break,
                };
                stream.write_all(&buf[..n]).unwrap();
            }
        });

        let io = MioIoRuntime::new();
        let socket = io.tcp_connect(serde_json::json!(addr)).unwrap();

        io.tcp_send(socket.clone(), serde_json::json!("one"))
            .unwrap();
        let resp = io.tcp_recv(socket.clone()).unwrap();
        assert_eq!(resp.as_str(), Some("one"));

        io.tcp_send(socket.clone(), serde_json::json!("two"))
            .unwrap();
        let resp = io.tcp_recv(socket.clone()).unwrap();
        assert_eq!(resp.as_str(), Some("two"));

        io.tcp_close(socket).unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn mio_tcp_close() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let _handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let _ = stream;
        });

        let io = MioIoRuntime::new();
        let socket = io.tcp_connect(serde_json::json!(addr)).unwrap();

        io.tcp_close(socket.clone()).unwrap();

        let res = io.tcp_send(socket, serde_json::json!("hello"));
        assert!(res.is_err(), "expected send on closed socket to error");
    }
}
