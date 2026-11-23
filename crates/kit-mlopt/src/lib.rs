//! `handl-kit-mlopt` — the mlopt meta-tier kit: pipeline stage-ops as a
//! native `pipeline` module for `hnl run` (ADR-0175 run layer).
//!
//! Stage-ops are ordinary `Value -> Value` host functions — surface
//! `a >>> f` is App scheme (`f(a)`), so `lift("demo.torch") >>> search >>>
//! verify` threads one JSON record stage to stage. Each stage forwards
//! its input to the mlopt backend and returns the backend's output
//! record:
//!
//! ```text
//!   <stage input JSON>  --stdin-->  $MLOPT_PIPELINE_CMD <stage>
//!                                        |
//!   <stage output JSON> <--stdout--    python -m mlopt stage <stage>
//! ```
//!
//! Command resolution (first match wins):
//!
//! 1. `MLOPT_PIPELINE_CMD` — a whitespace-separated command prefix; the
//!    stage name is appended as the final argument. This is the test /
//!    alternate-backend seam.
//! 2. `MLOPT_PYTHON -m mlopt stage` — the mlopt project's interpreter
//!    override (same convention as `hnl-ext-mlopt`).
//! 3. `python3 -m mlopt stage` — expects the mlopt venv activated or
//!    `mlopt` importable.
//!
//! **Protocol status: landed.** `python -m mlopt stage <name>` is the
//! per-stage JSON endpoint this kit speaks (`mlopt/src/mlopt/stage.py`,
//! wired in `cli.py`):
//!
//! > `python -m mlopt stage <lift|search|verify|deliver>` reads exactly
//! > one JSON value from stdin (the stage input: a model-ref string or
//! > `{"model": ..., "adapter_opts": {...}}` object for `lift`, the
//! > pipeline record object thereafter — `deliver` additionally wants
//! > the record's `out` artifact path), writes exactly one JSON value
//! > to stdout (the stage output record), and exits non-zero with
//! > diagnostics on stderr on failure.
//!
//! `deliver` is exposed as `pipeline.deliver` like the other stages.
//! Records carry the `"v"` schema pin (`mlopt.stage.RECORD_VERSION`,
//! ADR-0178 WP0a): this kit validates it on every stage input and
//! output — a missing `v` is the v0 legacy format (accepted), a
//! newer or malformed `v` is a loud [`EvalError`]. Its seam is the
//! record, not the signature: stage-ops are
//! `Value -> Value`, so the artifact destination cannot be a second
//! argument — the pipeline script sets the record's `out` field
//! between `verify` and `deliver` (e.g. an object-spread update
//! `r ~> {...r, "out": "delivered.hnl"}`), and `deliver` re-runs the
//! real `lower(verify=True)` gate before writing the `.hnl` artifact.
//! `MLOPT_PIPELINE_CMD` still lets a host point at any conforming
//! command (the test suite uses a fixture script).
//!
//! ## CLI command surface
//!
//! The same crate also carries the `hnl mlopt <cmd> <args…>` command
//! surface (dissolution plan Step B): [`run_mlopt`] spawns
//! `$MLOPT_PYTHON|-m mlopt <args>` verbatim — `models`, `optimize`,
//! `export`, `doctor`, and `stage` all forward to the Python CLI, which
//! remains the product edge. The `hnl-ext-mlopt` cdylib in this
//! workspace is the thin ADR-0149 ABI shim over it, replacing the
//! same-named crate that lived in the mlopt repo.
//!
//! ## Registration
//!
//! Same seam as every `kit-*` crate: [`MODULES`] + [`register`], called
//! by [`handl_kit_prelude::register`] (this kit is a dependency of
//! `handl-kit-prelude`). Visibility to `hnl run` is gated a second time
//! by the prelude manifest's `base_modules` — `pipeline` must be listed
//! there (kit-data-prelude `data/kits/prelude.yaml`) for
//! `build_stdlib_registry`'s `want` filter to admit it.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Once};

use handl_kit_core::{KitResolver, ModuleCtor, ModuleEntry};
use handl_platform::{EvalContext, EvalError, Function, Module};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Record schema pin — the `v` field every pipeline record carries
// ---------------------------------------------------------------------------

/// The pipeline-record schema version this kit speaks — mirrors
/// `mlopt.stage.RECORD_VERSION` (`mlopt/src/mlopt/stage.py`, the
/// ADR-0178 WP0a pin). Every stage record carries `"v": <int>`; a
/// record with no `v` is the v0 legacy format and is always accepted,
/// while a *higher* `v` is refused loudly — the kit never guesses at
/// a schema it does not know. Bump this when the kit learns a newer
/// record schema; it must never run ahead of what it validates.
const RECORD_SCHEMA_VERSION: i64 = 3;

/// Validate a record's `v` field — the tolerant reader, kit side.
///
/// Missing `v` is the v0 legacy format (accepted); an integer `v` up
/// to [`RECORD_SCHEMA_VERSION`] is accepted; a non-integer `v`
/// (bool/float/string) or a newer schema is a named [`EvalError`].
/// Non-object inputs (the `lift` model-ref string) carry no `v` and
/// pass through.
fn check_record_schema(rec: &Value) -> Result<(), EvalError> {
    let Some(v) = rec.get("v") else {
        return Ok(());
    };
    match v.as_i64() {
        // `as_i64` rejects bools and floats — exactly the malformed
        // spellings the mlopt reader declines too.
        Some(n) if n <= RECORD_SCHEMA_VERSION => Ok(()),
        Some(n) => Err(EvalError::Other(format!(
            "pipeline record 'v'={n} is newer than this kit's schema \
             (v{RECORD_SCHEMA_VERSION}) — upgrade handl-kit-mlopt or \
             regenerate the record"
        ))),
        None => Err(EvalError::Other(format!(
            "pipeline record 'v' must be an int schema version, got {v}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Stage functions — `Value -> Value` over the JSON stdin/stdout protocol
// ---------------------------------------------------------------------------

/// The stage name this function runs
/// (`"lift"` / `"search"` / `"verify"` / `"deliver"`).
#[derive(Clone, Copy)]
struct StageFn {
    stage: &'static str,
}

/// Resolve the command prefix (everything before the stage name).
///
/// Returns `(program, args-prefix)`. See the crate docs for the
/// resolution order.
fn stage_command_prefix() -> (String, Vec<String>) {
    if let Ok(spec) = std::env::var("MLOPT_PIPELINE_CMD") {
        let mut parts = spec.split_whitespace();
        if let Some(prog) = parts.next() {
            return (
                prog.to_string(),
                parts.map(str::to_string).collect::<Vec<_>>(),
            );
        }
    }
    if let Ok(py) = std::env::var("MLOPT_PYTHON") {
        return (py, vec!["-m".into(), "mlopt".into(), "stage".into()]);
    }
    (
        "python3".to_string(),
        vec!["-m".into(), "mlopt".into(), "stage".into()],
    )
}

/// Spawn `<prefix> <stage>`, write `input` as one JSON value on stdin,
/// and return the child's stdout parsed as one JSON value.
///
/// `argv_override` bypasses env resolution entirely (tests).
fn run_stage(
    stage: &str,
    input: &Value,
    argv_override: Option<&[String]>,
) -> Result<Value, EvalError> {
    // The schema pin gates both directions: an input record stamped
    // newer than this kit is refused before spawning (the backend
    // would decline it anyway — this names the cause earlier), and a
    // stage output is validated before it flows to the next stage.
    check_record_schema(input)?;
    let (prog, mut args) = match argv_override {
        Some(argv) => {
            let (p, rest) = argv
                .split_first()
                .ok_or_else(|| EvalError::Other("pipeline stage: empty command override".into()))?;
            (p.clone(), rest.to_vec())
        }
        None => stage_command_prefix(),
    };
    args.push(stage.to_string());

    let mut child = Command::new(&prog)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            EvalError::Other(format!(
                "pipeline.{stage}: failed to launch `{prog}` ({e}) — \
                 set MLOPT_PIPELINE_CMD to a `python -m mlopt stage`-conforming command"
            ))
        })?;

    // The input is one JSON value — small (a model ref or a pipeline
    // record), so a blocking write is fine; the child reads stdin to EOF.
    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_string(input)
            .map_err(|e| EvalError::Other(format!("pipeline.{stage}: cannot encode input: {e}")))?;
        // A child that exits early (bad args) closes stdin — EPIPE is a
        // stage failure reported via exit status below, not a panic.
        let _ = stdin.write_all(payload.as_bytes());
        let _ = stdin.write_all(b"\n");
    }

    let output = child.wait_with_output().map_err(|e| {
        EvalError::Other(format!("pipeline.{stage}: failed to wait on `{prog}`: {e}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr
            .lines()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(EvalError::Other(format!(
            "pipeline.{stage}: `{prog} {}` exited {}{}",
            args.join(" "),
            output.status,
            if tail.is_empty() {
                String::new()
            } else {
                format!(": {tail}")
            }
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let record: Value = serde_json::from_str(stdout.trim()).map_err(|e| {
        EvalError::Other(format!(
            "pipeline.{stage}: stdout is not one JSON value ({e}): {}",
            stdout.trim().chars().take(200).collect::<String>()
        ))
    })?;
    check_record_schema(&record)?;
    Ok(record)
}

impl Function for StageFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        // Stage-ops are `Value -> Value`: exactly one argument — the
        // stage input. Extra arguments have no protocol meaning
        // (`deliver`'s `out` rides inside the record), so they are an
        // error, never silently dropped.
        if args.len() > 1 {
            return Err(EvalError::Other(format!(
                "pipeline.{}: expected 1 argument (the stage input record), got {}",
                self.stage,
                args.len()
            )));
        }
        let input = args.first().cloned().unwrap_or(Value::Null);
        run_stage(self.stage, &input, None)
    }
}

// ---------------------------------------------------------------------------
// CLI command surface — `mlopt models|optimize|export|doctor|stage`
// ---------------------------------------------------------------------------
//
// Step B of the mlopt dissolution plan
// (`host/project/reports/mlopt_dissolution_plan.md`): the `hnl mlopt …`
// command surface forwards argv verbatim to `python -m mlopt <args>` —
// the same subprocess pattern the stage-ops use, with the Python
// package remaining the edge. `hnl` is a dumb ext host (ADR-0149), so
// the command reaches users through the `hnl-ext-mlopt` cdylib in this
// workspace, which is a thin ABI shim over [`run_mlopt`].

/// The outcome of a `python -m mlopt <args>` invocation — the same
/// `{code, stdout, stderr}` triple the ADR-0149 `ExtResponse` carries,
/// kept dependency-free here so the kit crate stays usable without the
/// FFI shim.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MloptResponse {
    /// Process exit code (`0` success; `2` when the interpreter could
    /// not be launched — the shim's usage-error convention).
    pub code: i32,
    /// The child's stdout, verbatim.
    pub stdout: String,
    /// The child's stderr, verbatim.
    pub stderr: String,
}

/// Resolve the interpreter for `python -m mlopt` invocations.
///
/// Order (first match wins): `MLOPT_PYTHON` in `env`, then
/// `MLOPT_PYTHON` in the process environment, then `python3` on PATH —
/// the same convention `hnl-ext-mlopt` and the stage-ops use.
fn mlopt_python(env: &BTreeMap<String, String>) -> String {
    env.get("MLOPT_PYTHON")
        .cloned()
        .or_else(|| std::env::var("MLOPT_PYTHON").ok())
        .unwrap_or_else(|| "python3".to_string())
}

/// Spawn `<python> -m mlopt <args>` in `cwd` and collect the result.
///
/// `args` is the argv *after* `mlopt` (e.g. `["optimize", "whisper:…"]`)
/// and is forwarded verbatim — the Python CLI owns subcommand parsing,
/// so `models`/`optimize`/`export`/`doctor`/`stage` all ride the same
/// path. `env` is the caller's environment view (the ext ABI passes the
/// host env verbatim); entries are applied on top of the inherited
/// environment, and `PYTHONUNBUFFERED=1` is always set so piped stdout
/// is not block-buffered.
///
/// This function is total: a spawn failure (missing interpreter,
/// `cwd` gone) returns `code: 2` with the diagnostic on `stderr`,
/// matching the `ExtResponse::error(2, …)` convention the host applies
/// no further meaning to.
pub fn run_mlopt(args: &[String], cwd: &Path, env: &BTreeMap<String, String>) -> MloptResponse {
    let python = mlopt_python(env);
    let mut argv: Vec<String> = vec!["-m".to_string(), "mlopt".to_string()];
    argv.extend(args.iter().cloned());

    let output = Command::new(&python)
        .args(&argv)
        .current_dir(cwd)
        .envs(env.iter())
        .env("PYTHONUNBUFFERED", "1")
        .output();

    match output {
        Ok(out) => MloptResponse {
            code: out.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        Err(e) => MloptResponse {
            code: 2,
            stdout: String::new(),
            stderr: format!(
                "mlopt: failed to launch `{python}` ({e}) — set MLOPT_PYTHON to the project's .venv interpreter\n"
            ),
        },
    }
}

// ---------------------------------------------------------------------------
// The `pipeline` module
// ---------------------------------------------------------------------------

/// `pipeline.lift` / `pipeline.search` / `pipeline.verify` /
/// `pipeline.deliver` — also reachable as flat wires (`lift`, `search`,
/// `verify`, `deliver`) through `ModuleLookup`'s all-module function
/// search.
struct PipelineModule {
    lift: StageFn,
    search: StageFn,
    verify: StageFn,
    deliver: StageFn,
}

impl PipelineModule {
    fn new() -> Self {
        PipelineModule {
            lift: StageFn { stage: "lift" },
            search: StageFn { stage: "search" },
            verify: StageFn { stage: "verify" },
            deliver: StageFn { stage: "deliver" },
        }
    }
}

impl Module for PipelineModule {
    fn name(&self) -> &str {
        "pipeline"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "lift" => Some(&self.lift as &dyn Function),
            "search" => Some(&self.search as &dyn Function),
            "verify" => Some(&self.verify as &dyn Function),
            "deliver" => Some(&self.deliver as &dyn Function),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![
            ("lift", &self.lift as &dyn Function),
            ("search", &self.search as &dyn Function),
            ("verify", &self.verify as &dyn Function),
            ("deliver", &self.deliver as &dyn Function),
        ]
    }
}

/// The kit-crate seam: `("pipeline", ctor)` in [`MODULES`], exactly the
/// shape `kit-test`/`kit-ffi`/`kit-stdlib` use.
fn pipeline_ctor(_resolver: &KitResolver, _eval_name: &'static str) -> ModuleEntry {
    ModuleEntry::Native(Arc::new(PipelineModule::new()))
}

/// This kit's module table.
pub const MODULES: &[(&'static str, ModuleCtor)] = &[("pipeline", pipeline_ctor)];

/// Register the kit's modules into the shared [`handl_kit_core`] table.
///
/// `Once`-safe and idempotent; `handl_kit_prelude::register` calls this
/// for every canonical-kit assembly. A host that links `handl-kit-mlopt`
/// directly calls it at its own entry point — importing this crate
/// mutates nothing.
pub fn register() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| handl_kit_core::register_modules(MODULES));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture stub: `tests/fixtures/stage_stub.py` — a Python endpoint
    /// that threads the JSON record per stage, fails on `explode`,
    /// prints non-JSON on `badjson`.
    fn stub_argv() -> Vec<String> {
        vec![
            "python3".to_string(),
            format!(
                "{}/tests/fixtures/stage_stub.py",
                env!("CARGO_MANIFEST_DIR")
            ),
        ]
    }

    fn call_with(stage: &'static str, input: Value) -> Result<Value, EvalError> {
        let argv = stub_argv();
        run_stage(stage, &input, Some(&argv))
    }

    #[test]
    fn lift_returns_stub_record() {
        let out = call_with("lift", Value::String("demo.torch".into())).unwrap();
        assert_eq!(out["stage"], "lift");
        assert_eq!(out["model"], "demo.torch");
        assert_eq!(out["ok"], true);
    }

    #[test]
    fn search_threads_record() {
        let lifted = call_with("lift", Value::String("demo.torch".into())).unwrap();
        let out = call_with("search", lifted).unwrap();
        assert_eq!(out["stage"], "search");
        assert_eq!(out["model"], "demo.torch");
        assert_eq!(out["laws_applied"], 7);
    }

    #[test]
    fn verify_gates_on_search() {
        let lifted = call_with("lift", Value::String("demo.torch".into())).unwrap();
        let searched = call_with("search", lifted).unwrap();
        let out = call_with("verify", searched).unwrap();
        assert_eq!(out["stage"], "verify");
        assert_eq!(out["ok"], true);
    }

    #[test]
    fn deliver_threads_record_and_writes_artifact_stub() {
        // The deliver stage wants the verify record's `verified` claim
        // plus the `out` artifact path — both ride the JSON record, so
        // a script sets `out` between verify and deliver (object
        // spread) and the stub echoes the artifact block.
        let mut rec = call_with("lift", Value::String("demo.torch".into())).unwrap();
        rec["verified"] = serde_json::json!({"passed": true});
        rec["out"] = serde_json::json!("/tmp/delivered.hnl");
        let out = call_with("deliver", rec).unwrap();
        assert_eq!(out["stage"], "deliver");
        assert_eq!(out["delivered"], true);
        assert_eq!(out["artifact"]["path"], "/tmp/delivered.hnl");
    }

    #[test]
    fn nonzero_exit_is_loud_eval_error() {
        let err = call_with("explode", Value::Null).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("explode"), "unexpected: {msg}");
        assert!(msg.contains("exited"), "unexpected: {msg}");
    }

    #[test]
    fn bad_json_stdout_is_loud_eval_error() {
        // `badjson` stage prints non-JSON on stdout, exit 0.
        let err = call_with("badjson", Value::Null).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("not one JSON value"), "unexpected: {msg}");
    }

    #[test]
    fn missing_program_is_loud_eval_error() {
        let err = run_stage(
            "lift",
            &Value::Null,
            Some(&["/nonexistent/pipeline-stub".to_string()]),
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("failed to launch"), "unexpected: {msg}");
    }

    #[test]
    fn stage_fn_rejects_extra_args() {
        // Stage-ops are `Value -> Value` — a second argument has no
        // protocol meaning and must error, never drop silently.
        let f = StageFn { stage: "lift" };
        let mut ctx = EvalContext::new();
        let err = f
            .call(&[Value::Null, Value::String("extra".into())], &mut ctx)
            .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("expected 1 argument"), "unexpected: {msg}");
    }

    #[test]
    fn record_schema_pin_is_validated() {
        // `vok` emits `"v": 3` — the current pin passes through.
        let out = call_with("vok", Value::Null).unwrap();
        assert_eq!(out["v"], 3);

        // A newer schema is a loud error on the OUTPUT side.
        let err = call_with("vfuture", Value::Null).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("newer than this kit's schema"),
            "unexpected: {msg}"
        );

        // A malformed (non-int) `v` is a loud error too.
        let err = call_with("vbad", Value::Null).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("must be an int schema version"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn record_schema_pin_gates_input_before_spawn() {
        // A record stamped newer than the kit's schema is refused
        // before the backend is even spawned.
        let err = run_stage(
            "search",
            &serde_json::json!({"model": "demo.torch", "v": 99}),
            Some(&stub_argv()),
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("newer than this kit's schema"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn env_cmd_prefix_appends_stage() {
        // MLOPT_PIPELINE_CMD=<stub> must behave identically to the
        // argv-override path — the stage name is the last argument.
        // SAFETY: edition-2024 `set_var` is unsafe; this test binary's
        // other tests never read the env (they use argv overrides), and
        // the write happens once before any spawn.
        unsafe {
            std::env::set_var(
                "MLOPT_PIPELINE_CMD",
                format!(
                    "python3 {}/tests/fixtures/stage_stub.py",
                    env!("CARGO_MANIFEST_DIR")
                ),
            );
        }
        let out = run_stage("lift", &Value::String("demo.torch".into()), None).unwrap();
        assert_eq!(out["model"], "demo.torch");
    }

    // -----------------------------------------------------------------
    // CLI command surface — `run_mlopt` argv forwarding
    // -----------------------------------------------------------------

    /// `/bin/echo` stands in for the interpreter: it prints the
    /// forwarded argv (`-m mlopt <args>`) verbatim, so no Python is
    /// needed. Passed through the `env` map — no process-env mutation.
    fn echo_env() -> BTreeMap<String, String> {
        BTreeMap::from([("MLOPT_PYTHON".to_string(), "/bin/echo".to_string())])
    }

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn run_mlopt_forwards_argv_verbatim() {
        let r = run_mlopt(&args(&["models", "--json"]), Path::new("."), &echo_env());
        assert_eq!(r.code, 0, "stderr: {}", r.stderr);
        // echo prints `-m mlopt models --json` — the verbatim forward.
        assert!(
            r.stdout.contains("-m mlopt models --json"),
            "stdout: {}",
            r.stdout
        );
    }

    #[test]
    fn run_mlopt_covers_every_cli_subcommand() {
        // The forwarding is subcommand-agnostic: models/optimize/export/
        // doctor/stage all ride the same `python -m mlopt <args>` path.
        for sub in ["models", "optimize", "export", "doctor", "stage"] {
            let r = run_mlopt(&args(&[sub]), Path::new("."), &echo_env());
            assert_eq!(r.code, 0, "{sub}: stderr {}", r.stderr);
            assert!(
                r.stdout.contains(&format!("-m mlopt {sub}")),
                "{sub}: stdout {}",
                r.stdout
            );
        }
    }

    #[test]
    fn run_mlopt_propagates_child_exit_code_and_stderr() {
        // /bin/false exits 1 with empty streams — the response carries
        // the real status through untouched.
        let env = BTreeMap::from([("MLOPT_PYTHON".to_string(), "/bin/false".to_string())]);
        let r = run_mlopt(&args(&["doctor"]), Path::new("."), &env);
        assert_eq!(r.code, 1);
    }

    #[test]
    fn run_mlopt_missing_interpreter_is_code_2() {
        let env = BTreeMap::from([(
            "MLOPT_PYTHON".to_string(),
            "/nonexistent/python".to_string(),
        )]);
        let r = run_mlopt(&args(&["models"]), Path::new("."), &env);
        assert_eq!(r.code, 2);
        assert!(
            r.stderr.contains("failed to launch"),
            "stderr: {}",
            r.stderr
        );
        assert!(r.stderr.contains("MLOPT_PYTHON"), "stderr: {}", r.stderr);
    }

    #[test]
    fn mlopt_python_prefers_env_then_process_then_python3() {
        // Env-map entry wins over everything.
        let env = BTreeMap::from([("MLOPT_PYTHON".to_string(), "/bin/echo".to_string())]);
        assert_eq!(mlopt_python(&env), "/bin/echo");
        // An empty env map falls back to the process env / python3 —
        // assert only that it resolves to *something* non-empty.
        assert!(!mlopt_python(&BTreeMap::new()).is_empty());
    }
}
