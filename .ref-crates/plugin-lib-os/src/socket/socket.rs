use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::error::OsError;

/// The handl source declaring the os `op` interface.
const OS_HNL: &str = include_str!("../../../../handl-packages/os/os.hnl");

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
///
/// The interface is declared in `os.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct delegates
/// [`Module`] lookups to the [`HandlModule`] built from the service.
pub struct SocketModule {
    module: HandlModule,
}

impl SocketModule {
    pub fn new(socket: impl Socket + 'static) -> Self {
        let mut registry = OpRegistry::new();
        register_socket_ops(&mut registry, socket);
        Self {
            module: HandlModule::new("socket", OS_HNL, &[])
                .with_ops(SOCKET_OPS, Arc::new(registry)),
        }
    }
}

impl Module for SocketModule {
    fn name(&self) -> &str {
        "socket"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.module.functions()
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

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `socket` module.
pub const SOCKET_OPS: &[&str] = &[
    "socket.connect",
    "socket.send",
    "socket.recv",
    "socket.close",
];

/// Register the `socket` ops into the given [`OpRegistry`], dispatching to the
/// given [`Socket`] implementation.
pub fn register_socket_ops(registry: &mut OpRegistry, socket: impl Socket + 'static) {
    let socket: Arc<dyn Socket> = Arc::new(socket);
    registry.register(
        "socket.connect",
        Arc::new(ConnectFn {
            socket: socket.clone(),
        }),
    );
    registry.register(
        "socket.send",
        Arc::new(SendFn {
            socket: socket.clone(),
        }),
    );
    registry.register(
        "socket.recv",
        Arc::new(RecvFn {
            socket: socket.clone(),
        }),
    );
    registry.register("socket.close", Arc::new(CloseFn { socket }));
}
