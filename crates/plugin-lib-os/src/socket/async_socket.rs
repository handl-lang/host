use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use handl_platform::{EvalError, Function, Module};

use crate::error::OsError;

/// A boxed, sendable future for async socket operations.
pub type AsyncSocketFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, OsError>> + Send + 'a>>;

/// Abstract async socket service.
///
/// Implementations:
/// - `handl-lib-os-std` — `MioSocket` backed by `mio` (Unix only).
pub trait AsyncSocket: Send + Sync {
    /// Asynchronously connect to `address` and return a handle.
    fn connect<'a>(&'a self, address: &'a str) -> AsyncSocketFuture<'a, i64>;

    /// Asynchronously send `data` on `handle` and return the bytes written.
    fn send<'a>(&'a self, handle: i64, data: &'a str) -> AsyncSocketFuture<'a, usize>;

    /// Asynchronously receive up to `length` bytes from `handle`.
    fn recv<'a>(&'a self, handle: i64, length: usize) -> AsyncSocketFuture<'a, String>;

    /// Asynchronously close `handle`.
    fn close<'a>(&'a self, handle: i64) -> AsyncSocketFuture<'a, ()>;
}

/// Module adapter for [`AsyncSocket`] — exposes `socket.*` functions.
///
/// **ADR 0040 gap:** The new non-generic `Module`/`Function` traits are
/// synchronous. Async socket operations need to be reworked to use the
/// `EvalContext::eval_async` path or a capability-based approach. For now
/// the async function impls are retained as standalone structs but are not
/// exposed through the `Module` trait (which has no `async_function` method
/// in the new system).
pub struct AsyncSocketModule {
    #[allow(dead_code)]
    connect: AsyncConnectFn,
    #[allow(dead_code)]
    send: AsyncSendFn,
    #[allow(dead_code)]
    recv: AsyncRecvFn,
    #[allow(dead_code)]
    close: AsyncCloseFn,
}

impl AsyncSocketModule {
    pub fn new(socket: impl AsyncSocket + 'static) -> Self {
        let socket = Arc::new(socket);
        AsyncSocketModule {
            connect: AsyncConnectFn {
                socket: socket.clone(),
            },
            send: AsyncSendFn {
                socket: socket.clone(),
            },
            recv: AsyncRecvFn {
                socket: socket.clone(),
            },
            close: AsyncCloseFn { socket },
        }
    }
}

impl Module for AsyncSocketModule {
    fn name(&self) -> &str {
        "socket"
    }

    fn function(&self, _name: &str) -> Option<&dyn Function> {
        None
    }
}

#[allow(dead_code)]
fn require_string_arg(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    args.first()
        .ok_or_else(|| EvalError::TypeMismatch("socket: expected 1 argument, got 0".to_string()))?
        .as_str()
        .ok_or_else(|| EvalError::TypeMismatch("socket: expected string argument".to_string()))
}

#[allow(dead_code)]
fn require_number_arg(args: &[serde_json::Value]) -> Result<i64, EvalError> {
    args.first()
        .ok_or_else(|| EvalError::TypeMismatch("socket: expected 1 argument, got 0".to_string()))?
        .as_f64()
        .ok_or_else(|| EvalError::TypeMismatch("socket: expected number argument".to_string()))
        .map(|n| n as i64)
}

#[allow(dead_code)]
struct AsyncConnectFn {
    socket: Arc<dyn AsyncSocket>,
}

impl AsyncConnectFn {
    #[allow(dead_code)]
    fn call<'a>(
        &'a self,
        args: &'a [serde_json::Value],
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, EvalError>> + 'a>> {
        Box::pin(async move {
            let address = require_string_arg(args)?;
            let handle = self
                .socket
                .connect(address)
                .await
                .map_err(|e| EvalError::Other(e.to_string()))?;
            Ok(serde_json::Value::from(handle))
        })
    }
}

#[allow(dead_code)]
struct AsyncSendFn {
    socket: Arc<dyn AsyncSocket>,
}

impl AsyncSendFn {
    #[allow(dead_code)]
    fn call<'a>(
        &'a self,
        args: &'a [serde_json::Value],
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, EvalError>> + 'a>> {
        Box::pin(async move {
            if args.len() < 2 {
                return Err(EvalError::TypeMismatch(format!(
                    "socket.send: expected 2 args, got {}",
                    args.len()
                )));
            }
            let handle = require_number_arg(args)?;
            let data = args[1]
                .as_str()
                .ok_or_else(|| {
                    EvalError::TypeMismatch("socket.send: expected string argument".to_string())
                })?
                .to_string();
            let sent = self
                .socket
                .send(handle, &data)
                .await
                .map_err(|e| EvalError::Other(e.to_string()))?;
            Ok(serde_json::Value::from(sent))
        })
    }
}

#[allow(dead_code)]
struct AsyncRecvFn {
    socket: Arc<dyn AsyncSocket>,
}

impl AsyncRecvFn {
    #[allow(dead_code)]
    fn call<'a>(
        &'a self,
        args: &'a [serde_json::Value],
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, EvalError>> + 'a>> {
        Box::pin(async move {
            if args.len() < 2 {
                return Err(EvalError::TypeMismatch(format!(
                    "socket.recv: expected 2 args, got {}",
                    args.len()
                )));
            }
            let handle = require_number_arg(args)?;
            let length = args[1].as_f64().ok_or_else(|| {
                EvalError::TypeMismatch("socket.recv: expected number argument".to_string())
            })? as usize;
            let data = self
                .socket
                .recv(handle, length)
                .await
                .map_err(|e| EvalError::Other(e.to_string()))?;
            Ok(serde_json::Value::from(data))
        })
    }
}

#[allow(dead_code)]
struct AsyncCloseFn {
    socket: Arc<dyn AsyncSocket>,
}

impl AsyncCloseFn {
    #[allow(dead_code)]
    fn call<'a>(
        &'a self,
        args: &'a [serde_json::Value],
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, EvalError>> + 'a>> {
        Box::pin(async move {
            let handle = require_number_arg(args)?;
            self.socket
                .close(handle)
                .await
                .map_err(|e| EvalError::Other(e.to_string()))?;
            Ok(serde_json::Value::Null)
        })
    }
}
