use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;

use crate::error::OsError;

/// Output from a command execution.
#[derive(Debug, Clone)]
pub struct CmdOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Abstract command execution service.
///
/// Provides a way to interact with the command line — spawn processes,
/// capture stdout/stderr, and check exit codes.
///
/// Implementations:
/// - `handl-lib-os-std` — backed by `std::process::Command`
/// - [`cmd_stub::StubCmd`] — canned outputs for testing
pub trait Cmd: Send + Sync {
    fn run(&self, program: &str, args: &[&str]) -> Result<CmdOutput, OsError>;
}

/// Module adapter for [`Cmd`] — exposes `cmd.*` functions.
pub struct CmdModule {
    run: RunFn,
}

impl CmdModule {
    pub fn new(command: impl Cmd + 'static) -> Self {
        CmdModule {
            run: RunFn {
                command: Arc::new(command),
            },
        }
    }
}

impl Module for CmdModule {
    fn name(&self) -> &str {
        "cmd"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "run" => Some(&self.run),
            _ => None,
        }
    }
}

fn output_to_value(output: CmdOutput) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!({
        "exit_code": output.exit_code,
        "stdout": output.stdout,
        "stderr": output.stderr}))
}

struct RunFn {
    command: Arc<dyn Cmd>,
}

impl Function for RunFn {
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
        self.command
            .run(program, &arg_refs)
            .map(output_to_value)
            .map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `cmd` module.
pub const CMD_OPS: &[&str] = &[
    "cmd.run",
];

/// Register the `cmd` ops into the given [`OpRegistry`], dispatching to the
/// given [`Cmd`] implementation.
pub fn register_cmd_ops(registry: &mut OpRegistry, cmd: impl Cmd + 'static) {
    let command: Arc<dyn Cmd> = Arc::new(cmd);
    registry.register("cmd.run", Arc::new(RunFn { command }));
}
