use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};

use crate::error::OsError;

/// Abstract socket service.
///
/// Implementations:
/// - `handl-lib-os-std` — backed by `std::net`
/// - [`socket_stub::StubSocket`] — in-memory for testing
pub trait Socket: Send + Sync {
    fn connect(&self, address: &str) -> Result<i64, OsError>;
    fn send(&self, handle: i64, data: &str) -> Result<usize, OsError>;
    fn recv(&self, handle: i64, length: usize) -> Result<String, OsError>;
    fn close(&self, handle: i64) -> Result<(), OsError>;
}

/// Module adapter for [`Socket`] — exposes `socket.*` functions.
pub struct SocketModule {
    connect: ConnectFn,
    send: SendFn,
    recv: RecvFn,
    close: CloseFn,
}

impl SocketModule {
    pub fn new(socket: impl Socket + 'static) -> Self {
        let socket = Arc::new(socket);
        SocketModule {
            connect: ConnectFn {
                socket: socket.clone(),
            },
            send: SendFn {
                socket: socket.clone(),
            },
            recv: RecvFn {
                socket: socket.clone(),
            },
            close: CloseFn { socket },
        }
    }
}

impl Module for SocketModule {
    fn name(&self) -> &str {
        "socket"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "connect" => Some(&self.connect),
            "send" => Some(&self.send),
            "recv" => Some(&self.recv),
            "close" => Some(&self.close),
            _ => None,
        }
    }
}

fn require_string_arg(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    args.first()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?
        .as_str()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))
}

fn require_number_arg(args: &[serde_json::Value]) -> Result<i64, EvalError> {
    let n = args
        .first()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?
        .as_f64()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))?;
    Ok(n as i64)
}

struct ConnectFn {
    socket: Arc<dyn Socket>,
}

impl Function for ConnectFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let address = require_string_arg(args)?;
        self.socket
            .connect(address)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct SendFn {
    socket: Arc<dyn Socket>,
}

impl Function for SendFn {
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
        let handle = require_number_arg(args)?;
        let data = args[1].as_str().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))?;
        self.socket
            .send(handle, data)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct RecvFn {
    socket: Arc<dyn Socket>,
}

impl Function for RecvFn {
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
        let handle = require_number_arg(args)?;
        let length = args[1].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))? as usize;
        self.socket
            .recv(handle, length)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

struct CloseFn {
    socket: Arc<dyn Socket>,
}

impl Function for CloseFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let handle = require_number_arg(args)?;
        self.socket.close(handle)?;
        Ok(serde_json::Value::Null)
    }
}
