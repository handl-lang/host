use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use handl_vm_values_json::{Error, NonFunctionValue};
use parking_lot::Mutex;
use serde_json::json;

/// Backend trait for blocking async I/O primitives.
pub trait IoRuntime: Send + Sync {
    /// Open a TCP connection to `addr` and return a socket handle.
    fn tcp_connect(&self, addr: NonFunctionValue) -> Result<NonFunctionValue, Error>;

    /// Send `data` over `socket`.
    fn tcp_send(&self, socket: NonFunctionValue, data: NonFunctionValue) -> Result<NonFunctionValue, Error>;

    /// Receive up to 4096 bytes from `socket`.
    fn tcp_recv(&self, socket: NonFunctionValue) -> Result<NonFunctionValue, Error>;

    /// Close `socket`.
    fn tcp_close(&self, socket: NonFunctionValue) -> Result<NonFunctionValue, Error>;
}

fn socket_id(socket: &NonFunctionValue) -> Result<u64, Error> {
    socket
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::PrimitiveError("expected a tcp_socket value".to_string()))
}

fn tcp_socket_value(id: u64) -> NonFunctionValue {
    NonFunctionValue::new(json!({"type": "tcp_socket", "id": id}))
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
    fn tcp_connect(&self, addr: NonFunctionValue) -> Result<NonFunctionValue, Error> {
        let addr = addr
            .as_str()
            .ok_or_else(|| Error::PrimitiveError("tcp_connect expected a host:port string".to_string()))?;

        let stream = TcpStream::connect(addr)
            .map_err(|e| Error::PrimitiveError(e.to_string()))?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.streams.lock().insert(id, stream);

        Ok(tcp_socket_value(id))
    }

    fn tcp_send(&self, socket: NonFunctionValue, data: NonFunctionValue) -> Result<NonFunctionValue, Error> {
        let id = socket_id(&socket)?;
        let mut streams = self.streams.lock();
        let stream = streams
            .get_mut(&id)
            .ok_or_else(|| Error::PrimitiveError("tcp_send: unknown socket".to_string()))?;

        let payload = data
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| data.inner().to_string());

        stream
            .write_all(payload.as_bytes())
            .map_err(|e| Error::PrimitiveError(e.to_string()))?;

        Ok(NonFunctionValue::new(json!(null)))
    }

    fn tcp_recv(&self, socket: NonFunctionValue) -> Result<NonFunctionValue, Error> {
        let id = socket_id(&socket)?;
        let mut streams = self.streams.lock();
        let stream = streams
            .get_mut(&id)
            .ok_or_else(|| Error::PrimitiveError("tcp_recv: unknown socket".to_string()))?;

        let mut buf = [0u8; 4096];
        let n = stream
            .read(&mut buf)
            .map_err(|e| Error::PrimitiveError(e.to_string()))?;

        let text = String::from_utf8_lossy(&buf[..n]).to_string();
        Ok(NonFunctionValue::new(json!(text)))
    }

    fn tcp_close(&self, socket: NonFunctionValue) -> Result<NonFunctionValue, Error> {
        let id = socket_id(&socket)?;
        let mut streams = self.streams.lock();
        streams.remove(&id);
        Ok(NonFunctionValue::new(json!(null)))
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
        let socket = io
            .tcp_connect(NonFunctionValue::new(json!(addr)))
            .unwrap();

        io.tcp_send(socket.clone(), NonFunctionValue::new(json!("hello")))
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
        let res = io.tcp_connect(NonFunctionValue::new(json!(addr)));
        assert!(res.is_err(), "expected connection refused");
    }
}
