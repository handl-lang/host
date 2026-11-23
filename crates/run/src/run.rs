//! `Runner` — the `hnl run <name>` orchestrator (P-003).
//!
//! Resolves the `dependsOn` DAG, executes tasks in topo order
//! (sequentially in v1), routes object-form tasks through the
//! fingerprinted cache, and propagates the first failure. String/array
//! scripts always run — they have no place to declare `inputs`/
//! `outputs`, so they are never cached in v1.
//!
//! Stream policy: child stdout/stderr are captured per script and echoed
//! to this process's streams (and recorded in [`RunReport`]) while being
//! stored for the cache entry; on a hit the recorded streams are
//! written out verbatim and the recorded exit code returned.

use std::io::Write;

use crate::cache::{self, CacheCheck, Toolchain};
use crate::error::RunError;
use crate::exec::{self, ExecOutcome, MAX_RUN_DEPTH};
use crate::pkg::PkgScripts;
use crate::sched::resolve_order;
use crate::script::Script;

pub use crate::cache::MissReason;

/// What the runner was asked to do about caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// Cache lookups and stores both enabled (default).
    Enabled,
    /// `--no-cache` — bypass lookup *and* store.
    Disabled,
}

/// Knobs for one [`Runner::run`] / [`Runner::plan`] call.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// `--no-cache` equivalent.
    pub cache: CacheMode,
    /// Toolchain id folded into cache keys (default
    /// [`Toolchain::detect`]).
    pub toolchain: Toolchain,
    /// `HANDL_RUN_DEPTH` override — `None` reads the env var. Tests use
    /// this to exercise the cap without mutating process env.
    pub depth: Option<u32>,
    /// Write child streams to this process's stdout/stderr *live* while
    /// recording them in the report (default `true`). `false` captures
    /// only — for embedders (the `hnl` extension ABI) that return the
    /// streams through their own response channel and must keep the
    /// process streams clean (e.g. for `--json` output).
    pub echo: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            cache: CacheMode::Enabled,
            toolchain: Toolchain::detect(),
            depth: None,
            echo: true,
        }
    }
}

/// The runner — cheap to construct, holds no per-run state.
#[derive(Debug, Clone, Default)]
pub struct Runner {
    /// Resolved options (default = [`RunOptions::default`]).
    pub options: RunOptions,
}

/// Per-task outcome inside a [`RunReport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Executed (cache miss or uncached); carries the exit code.
    Ran {
        /// Process exit code of the last executed segment.
        code: i32,
    },
    /// Served from the cache — streams replayed, outputs restored.
    CacheHit {
        /// Recorded exit code (always 0 — failures are never cached).
        code: i32,
    },
    /// Not executed because an earlier task failed.
    Skipped,
}

/// What happened to one task in the resolved order.
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// Script name.
    pub name: String,
    /// Outcome.
    pub status: TaskStatus,
    /// For a cache miss, the named culprit; `None` for hits/uncached.
    pub miss_reason: Option<String>,
}

/// The result of [`Runner::run`].
#[derive(Debug, Default)]
pub struct RunReport {
    /// The script that was invoked.
    pub script: String,
    /// Resolved execution order (deps first).
    pub order: Vec<String>,
    /// Per-task results, in execution order.
    pub tasks: Vec<TaskResult>,
    /// First nonzero exit code, or 0.
    pub exit_code: i32,
    /// Human-readable cache/verdict lines (`cache miss: …`,
    /// `cache hit: …`, notices) — ext-eval prints these to stderr.
    pub messages: Vec<String>,
    /// Everything emitted on stdout — live echo of miss-runs plus
    /// verbatim replay of hits. For `--json`/tests.
    pub stdout: Vec<u8>,
    /// Everything emitted on stderr.
    pub stderr: Vec<u8>,
}

/// The verdict for one task in [`PlanReport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanVerdict {
    /// Cache entry validates — would replay.
    Hit,
    /// Would miss; the string names the culprit.
    Miss(String),
    /// Not cached (script form, `cache: false`, or `--no-cache`).
    Uncached,
}

/// One row of [`PlanReport`].
#[derive(Debug, Clone)]
pub struct PlannedTask {
    /// Script name.
    pub name: String,
    /// Cache verdict.
    pub verdict: PlanVerdict,
}

/// The result of [`Runner::plan`] — the resolved DAG plus per-task
/// cache verdicts, computed *without* executing (the full fingerprint
/// pass runs, so verdicts are real; dep outputs are not restored first,
/// so a dependent whose inputs cover a dep's outputs may over-report a
/// miss — conservative, per P-003).
#[derive(Debug)]
pub struct PlanReport {
    /// The script that was planned.
    pub script: String,
    /// Resolved execution order.
    pub order: Vec<String>,
    /// Per-task verdicts in execution order.
    pub tasks: Vec<PlannedTask>,
}

impl Runner {
    /// A runner with default options.
    pub fn new() -> Self {
        Runner::default()
    }

    /// A runner with explicit options.
    pub fn with_options(options: RunOptions) -> Self {
        Runner { options }
    }

    /// Run `name` plus its `dependsOn` closure. `args` append to the
    /// named script's last command argv (deps run bare).
    ///
    /// # Errors
    ///
    /// Resolution/scheduling/exec diagnostics — see [`RunError`]. A
    /// *task failure* (nonzero exit) is not an error: it is reported in
    /// [`RunReport::exit_code`] with dependents [`TaskStatus::Skipped`].
    pub fn run(
        &self,
        pkg: &PkgScripts,
        name: &str,
        args: &[String],
    ) -> Result<RunReport, RunError> {
        let depth = self.options.depth.unwrap_or_else(exec::current_depth);
        if depth >= MAX_RUN_DEPTH {
            return Err(RunError::DepthExceeded { max: MAX_RUN_DEPTH });
        }
        let order = resolve_order(pkg, name)?;
        let mut report = RunReport {
            script: name.to_string(),
            order: order.clone(),
            ..RunReport::default()
        };
        let mut failed = false;

        for task_name in &order {
            let script = pkg.get(task_name).expect("resolve_order validated");
            // CLI args append only to the *invoked* script's last
            // command — deps run bare.
            let script_args: &[String] = if task_name == name { args } else { &[] };

            if failed {
                report.tasks.push(TaskResult {
                    name: task_name.clone(),
                    status: TaskStatus::Skipped,
                    miss_reason: None,
                });
                continue;
            }

            let (status, miss_reason) = self.run_one(
                pkg,
                script,
                script_args,
                depth,
                &mut report.messages,
                &mut report.stdout,
                &mut report.stderr,
            )?;
            if let TaskStatus::Ran { code } = status
                && code != 0
            {
                failed = true;
                report.exit_code = code;
            }
            report.tasks.push(TaskResult {
                name: task_name.clone(),
                status,
                miss_reason,
            });
        }
        Ok(report)
    }

    /// `--plan`: resolve the DAG and report each task's cache verdict
    /// without executing. Costs the fingerprint pass, not the run.
    pub fn plan(
        &self,
        pkg: &PkgScripts,
        name: &str,
        args: &[String],
    ) -> Result<PlanReport, RunError> {
        let order = resolve_order(pkg, name)?;
        let mut report = PlanReport {
            script: name.to_string(),
            order: order.clone(),
            tasks: Vec::new(),
        };
        for task_name in &order {
            let script = pkg.get(task_name).expect("resolve_order validated");
            let script_args: &[String] = if task_name == name { args } else { &[] };
            let verdict = self.plan_one(pkg, script, script_args)?;
            report.tasks.push(PlannedTask {
                name: task_name.clone(),
                verdict,
            });
        }
        Ok(report)
    }

    /// Execute (or replay) one script in the resolved order.
    #[allow(clippy::too_many_arguments)]
    fn run_one(
        &self,
        pkg: &PkgScripts,
        script: &Script,
        args: &[String],
        depth: u32,
        messages: &mut Vec<String>,
        out_stdout: &mut Vec<u8>,
        out_stderr: &mut Vec<u8>,
    ) -> Result<(TaskStatus, Option<String>), RunError> {
        let task = script.task();
        let cacheable = self.options.cache == CacheMode::Enabled && task.is_some_and(|t| t.cache);

        if let (Some(task), true) = (task, cacheable) {
            match cache::check(pkg, &script.name, task, args, &self.options.toolchain)? {
                CacheCheck::Hit { entry, meta } => {
                    let restored = cache::restore_missing_outputs(pkg, &entry)?;
                    let (stdout, stderr) = cache::read_streams(&entry);
                    emit_record(&stdout, true, out_stdout, self.options.echo)?;
                    emit_record(&stderr, false, out_stderr, self.options.echo)?;
                    if task.outputs.as_ref().is_none_or(|o| o.is_empty()) {
                        messages.push(format!(
                            "cache hit: {} (streams only — no outputs declared)",
                            script.name
                        ));
                    }
                    if !restored.is_empty() {
                        messages.push(format!("cache hit: restored {}", restored.join(", ")));
                    }
                    messages.push(format!("cache hit: {}", script.name));
                    return Ok((TaskStatus::CacheHit { code: meta.code }, None));
                }
                CacheCheck::Miss(reason) => {
                    messages.push(format!("cache miss: {reason}, executing"));
                    let outcome =
                        self.exec_script(pkg, script, args, depth, out_stdout, out_stderr)?;
                    if outcome.code == 0
                        && let Err(e) =
                            cache::store(pkg, script, task, args, &self.options.toolchain, &outcome)
                    {
                        messages.push(format!("warning: cannot write cache entry: {e}"));
                    }
                    return Ok((
                        TaskStatus::Ran { code: outcome.code },
                        Some(reason.to_string()),
                    ));
                }
            }
        }

        // Uncached path: scripts/arrays always run; `cache: false` and
        // `--no-cache` bypass lookup *and* store.
        let outcome = self.exec_script(pkg, script, args, depth, out_stdout, out_stderr)?;
        Ok((TaskStatus::Ran { code: outcome.code }, None))
    }

    /// The cache verdict for `--plan` — same check as execution, no run.
    fn plan_one(
        &self,
        pkg: &PkgScripts,
        script: &Script,
        args: &[String],
    ) -> Result<PlanVerdict, RunError> {
        let cacheable =
            self.options.cache == CacheMode::Enabled && script.task().is_some_and(|t| t.cache);
        let Some(task) = script.task() else {
            return Ok(PlanVerdict::Uncached);
        };
        if !cacheable {
            return Ok(PlanVerdict::Uncached);
        }
        match cache::check(pkg, &script.name, task, args, &self.options.toolchain)? {
            CacheCheck::Hit { .. } => Ok(PlanVerdict::Hit),
            CacheCheck::Miss(reason) => Ok(PlanVerdict::Miss(reason.to_string())),
        }
    }

    /// Build segments, execute them, and echo the captured streams to
    /// this process's stdout/stderr while recording them in the report
    /// (per-task granularity — child output is captured for the cache
    /// entry regardless).
    fn exec_script(
        &self,
        pkg: &PkgScripts,
        script: &Script,
        args: &[String],
        depth: u32,
        out_stdout: &mut Vec<u8>,
        out_stderr: &mut Vec<u8>,
    ) -> Result<ExecOutcome, RunError> {
        let segments = script.segments(args)?;
        let cwd = match script.task() {
            Some(task) => exec::task_cwd(&pkg.dir, &script.name, task.cwd.as_deref())?,
            None => pkg.dir.clone(),
        };
        let outcome = exec::exec_segments(&segments, &cwd, depth)?;
        emit_record(&outcome.stdout, true, out_stdout, self.options.echo)?;
        emit_record(&outcome.stderr, false, out_stderr, self.options.echo)?;
        Ok(outcome)
    }
}

/// Write captured bytes to the real stdout/stderr (when `echo`) *and*
/// append them to the report buffer — used both for live echo of a
/// miss-run's streams and for replaying a hit's recorded streams.
/// `echo: false` (see [`RunOptions::echo`]) records without writing.
fn emit_record(
    bytes: &[u8],
    stdout: bool,
    buf: &mut Vec<u8>,
    echo: bool,
) -> Result<(), RunError> {
    if bytes.is_empty() {
        return Ok(());
    }
    if echo {
        let result = if stdout {
            std::io::stdout().lock().write_all(bytes)
        } else {
            std::io::stderr().lock().write_all(bytes)
        };
        result.map_err(|e| {
            RunError::io(format!(
                "cannot write {}: {e}",
                if stdout { "stdout" } else { "stderr" }
            ))
        })?;
    }
    buf.extend_from_slice(bytes);
    Ok(())
}

/// One-line usage for `hnl run` (no args): `name — first command…`.
pub fn usage_line(script: &Script) -> String {
    let first = script.commands().first().map(|c| c.trim()).unwrap_or("");
    let summary = if first.chars().count() > 60 {
        let mut s: String = first.chars().take(57).collect();
        s.push('…');
        s
    } else {
        first.to_string()
    };
    if summary.is_empty() {
        script.name.clone()
    } else {
        format!("{} — {summary}", script.name)
    }
}

/// All declared scripts' usage lines, sorted by name — the `hnl run`
/// listing.
pub fn list_scripts(pkg: &PkgScripts) -> Vec<String> {
    let mut lines: Vec<String> = pkg.scripts.values().map(usage_line).collect();
    lines.sort();
    lines
}
