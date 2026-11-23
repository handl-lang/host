//! `handl-run` — the `hnl run <script>` task runner core (proposal P-003).
//!
//! `pkg.hnl` gains a `scripts:` table (scanned by
//! [`handl_platform::config::pkg_scripts`]); this crate resolves script
//! names walking up from cwd, topo-sorts `dependsOn`, executes argv
//! (no shell), and caches object-form tasks under
//! `.handl/cache/run/<key>/` with fingerprinted inputs, named miss
//! reasons, output-drift refusal, and stream replay.
//!
//! Plain library, no ext ABI — `plugin-ext-eval`'s `hnl run` command
//! consumes it for the script-name dispatch (the file-execution path is
//! unchanged).
//!
//! # Example
//!
//! ```no_run
//! use handl_run::{PkgScripts, Runner, RunOptions, CacheMode};
//!
//! let pkg = PkgScripts::discover(std::path::Path::new(".")).unwrap();
//! let runner = Runner::with_options(RunOptions {
//!     cache: CacheMode::Enabled,
//!     ..RunOptions::default()
//! });
//! let report = runner.run(&pkg, "ci", &[]).unwrap();
//! std::process::exit(report.exit_code);
//! ```

#![warn(missing_docs)]

/// Fingerprinted cache store (`.handl/cache/run/`).
pub mod cache;
/// Error types.
pub mod error;
/// argv execution, `hnl` argv[0] resolution, `HANDL_RUN_DEPTH`.
pub mod exec;
/// `PkgScripts` — manifest loading and discovery.
pub mod pkg;
/// `Runner`, reports, plan verdicts, listing.
pub mod run;
/// `dependsOn` topo sort.
pub mod sched;
/// Script/task model and argv splitting.
pub mod script;

pub use cache::{CacheCheck, EntryMeta, InputRecord, MissReason, Toolchain, check, entry_key};
pub use error::RunError;
pub use exec::{
    DEPTH_ENV, ExecOutcome, HNL_ENV, MAX_RUN_DEPTH, current_depth, depth_env_for, exec_segments,
    is_hnl_host, is_script_invocation, resolve_hnl_argv0, resolve_hnl_program, task_cwd,
};
pub use pkg::PkgScripts;
pub use run::{
    CacheMode, PlanReport, PlanVerdict, PlannedTask, RunOptions, RunReport, Runner, TaskResult,
    TaskStatus, list_scripts, usage_line,
};
pub use sched::{check_acyclic, dependency_map, resolve_all, resolve_order};
pub use script::{Argv, Script, ScriptBody, Task, canonical_task};
