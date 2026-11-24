//! Rust `Function` impls for the `proc` module (P-012) — host atoms for
//! process identity: own argv, exit code, pid.
//!
//! These are runtime/launch data, not POSIX functions (`lib-posix` stays
//! strict POSIX.1), so they live in the host stdlib table rather than a
//! libc backend. `argv`/`pid` read `std::env`/`std::process`; `exit`
//! terminates the process and never returns (the `-> a` contract,
//! same convention as `error.raise`).

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};

use crate::shared::{arr, num};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn no_args(op: &str, args: &[serde_json::Value]) -> Result<(), EvalError> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(EvalError::TypeMismatch(format!(
            "{op}: expected 0 args, got {}",
            args.len(),
        )))
    }
}

fn require_exit_code(args: &[serde_json::Value]) -> Result<i32, EvalError> {
    args.first()
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        .ok_or_else(|| EvalError::TypeMismatch("proc.exit: expected Int exit code".to_string()))
}

// ---------------------------------------------------------------------------
// Ops
// ---------------------------------------------------------------------------

/// `proc.argv` — own-process argument vector, argv[0] included.
#[derive(Clone, Copy)]
pub struct Argv;
impl Function for Argv {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        no_args("proc.argv", args)?;
        let argv: Vec<serde_json::Value> = std::env::args().map(serde_json::Value::from).collect();
        Ok(arr(&argv))
    }
}

/// `proc.exit` — terminate the process with the given exit code. Never
/// returns; the `!` diverges before `call` can produce a `Result`.
#[derive(Clone, Copy)]
pub struct Exit;
impl Function for Exit {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let code = require_exit_code(args)?;
        std::process::exit(code);
    }
}

/// `proc.pid` — own process id.
#[derive(Clone, Copy)]
pub struct Pid;
impl Function for Pid {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        no_args("proc.pid", args)?;
        Ok(num(std::process::id() as f64))
    }
}

// ---------------------------------------------------------------------------
// Impl table (P-013 T-149)
// ---------------------------------------------------------------------------

/// The `impl:` table `lib-proc/atoms.yaml` keys into — `impl` name →
/// `Function` impl. The atoms spec (`module: proc`, `proc.argv` /
/// `proc.exit` / `proc.pid`) plus this table fully determine the
/// module's `handl_plugin_lib_handl::OpRegistry`;
/// [`crate::atoms::registry_from_atoms`] does
/// the merge. There is no `*_op_registry()`/`*_OPS` hand-wiring left —
/// adding an atom is one yaml line + one impl fn + one row here.
pub fn proc_impls() -> Vec<(&'static str, Arc<dyn Function + Send + Sync>)> {
    vec![
        ("proc_argv", Arc::new(Argv)),
        ("proc_exit", Arc::new(Exit)),
        ("proc_pid", Arc::new(Pid)),
    ]
}
