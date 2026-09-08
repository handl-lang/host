use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::error::OsError;

/// The handl source declaring the os `op` interface.
const OS_HNL: &str = include_str!("../../../../handl-packages/os/os.hnl");

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
///
/// The interface is declared in `os.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct delegates
/// [`Module`] lookups to the [`HandlModule`] built from the service.
pub struct CmdModule {
    module: HandlModule,
}

impl CmdModule {
    pub fn new(command: impl Cmd + 'static) -> Self {
        let mut registry = OpRegistry::new();
        register_cmd_ops(&mut registry, command);
        Self {
            module: HandlModule::new("cmd", OS_HNL, &[]).with_ops(CMD_OPS, Arc::new(registry)),
        }
    }
}

impl Module for CmdModule {
    fn name(&self) -> &str {
        self.module.name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.module.functions()
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
pub const CMD_OPS: &[&str] = &["cmd.run"];

/// Register the `cmd` ops into the given [`OpRegistry`], dispatching to the
/// given [`Cmd`] implementation.
pub fn register_cmd_ops(registry: &mut OpRegistry, cmd: impl Cmd + 'static) {
    let command: Arc<dyn Cmd> = Arc::new(cmd);
    registry.register("cmd.run", Arc::new(RunFn { command }));
}
