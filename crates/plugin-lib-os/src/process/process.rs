use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;

use crate::error::OsError;

/// Snapshot of a managed child process.
#[derive(Debug, Clone)]
pub struct ProcessStatus {
    pub process_id: i64,
    pub running: bool,
    pub exit_code: Option<i32>,
}

/// Abstract child-process management service.
///
/// Provides lifecycle control over OS processes: spawn, kill, wait, and poll.
///
/// Implementations:
/// - `handl-lib-os-std` — backed by `std::process`
/// - [`process_stub::StubProcess`] — canned process lifecycle for testing
pub trait Process: Send + Sync {
    fn spawn(&self, program: &str, args: &[&str]) -> Result<i64, OsError>;
    fn kill(&self, process_id: i64) -> Result<(), OsError>;
    fn wait(&self, process_id: i64) -> Result<i32, OsError>;
    fn status(&self, process_id: i64) -> Result<ProcessStatus, OsError>;
}

/// Module adapter for [`Process`] — exposes `process.*` functions.
pub struct ProcessModule {
    spawn: SpawnFn,
    kill: KillFn,
    wait: WaitFn,
    status: StatusFn,
}

impl ProcessModule {
    pub fn new(process: impl Process + 'static) -> Self {
        let process = Arc::new(process);
        ProcessModule {
            spawn: SpawnFn {
                process: process.clone(),
            },
            kill: KillFn {
                process: process.clone(),
            },
            wait: WaitFn {
                process: process.clone(),
            },
            status: StatusFn { process },
        }
    }
}

impl Module for ProcessModule {
    fn name(&self) -> &str {
        "process"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "spawn" => Some(&self.spawn),
            "kill" => Some(&self.kill),
            "wait" => Some(&self.wait),
            "status" => Some(&self.status),
            _ => None,
        }
    }
}

fn status_to_value(status: ProcessStatus) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!({
        "process_id": status.process_id,
        "running": status.running,
        "exit_code": status.exit_code,
    }))
}

struct SpawnFn {
    process: Arc<dyn Process>,
}

impl Function for SpawnFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let program = args.first().ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )))?;
        let program = program.as_str().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))?;

        let arg_strings: Vec<String> = args
            .iter()
            .skip(1)
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or(EvalError::TypeMismatch(format!(
                        "expected {}, got {}",
                        "string", "non-string",
                    )))
            })
            .collect::<Result<_, _>>()?;
        let arg_refs: Vec<&str> = arg_strings.iter().map(String::as_str).collect();

        self.process
            .spawn(program, &arg_refs)
            .map(|id| serde_json::Value::from(serde_json::json!(id)))
            .map_err(Into::into)
    }
}

struct KillFn {
    process: Arc<dyn Process>,
}

impl Function for KillFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.is_empty() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1, 0,
            )));
        }
        let process_id = args[0].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))? as i64;
        self.process.kill(process_id)?;
        Ok(serde_json::Value::Null)
    }
}

struct WaitFn {
    process: Arc<dyn Process>,
}

impl Function for WaitFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.is_empty() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1, 0,
            )));
        }
        let process_id = args[0].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))? as i64;
        self.process
            .wait(process_id)
            .map(|code| serde_json::Value::from(serde_json::json!(code)))
            .map_err(Into::into)
    }
}

struct StatusFn {
    process: Arc<dyn Process>,
}

impl Function for StatusFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.is_empty() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1, 0,
            )));
        }
        let process_id = args[0].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))? as i64;
        self.process
            .status(process_id)
            .map(status_to_value)
            .map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `process` module.
pub const PROCESS_OPS: &[&str] = &[
    "process.spawn",
    "process.kill",
    "process.wait",
    "process.status",
];

/// Register the `process` ops into the given [`OpRegistry`], dispatching to the
/// given [`Process`] implementation.
pub fn register_process_ops(registry: &mut OpRegistry, process: impl Process + 'static) {
    let process: Arc<dyn Process> = Arc::new(process);
    registry.register("process.spawn", Arc::new(SpawnFn { process: process.clone() }));
    registry.register("process.kill", Arc::new(KillFn { process: process.clone() }));
    registry.register("process.wait", Arc::new(WaitFn { process: process.clone() }));
    registry.register("process.status", Arc::new(StatusFn { process }));
}
