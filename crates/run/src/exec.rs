//! argv execution — `std::process::Command`, no shell.
//!
//! `hnl` as argv[0] resolves to `current_exe()` **only when that binary
//! is the `hnl` host** (file name `hnl`/`hnl.exe`); otherwise —
//! `cargo test`, embedding, renamed binaries — it falls back to
//! `$HANDL_HNL` then `PATH`. Every other argv[0] resolves via `PATH`
//! normally (P-003).
//!
//! Recursion: a script may invoke `hnl run <other-name>`; the spawn
//! carries `HANDL_RUN_DEPTH` incremented per nested *script-name* run
//! (file execution `hnl run x.hnl` does not count — it cannot reach the
//! script runner without going through a script spawn). The variable is
//! inherited by all children but never fingerprinted unless a task
//! declares it in `env:`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Argv;
use crate::error::RunError;

/// Environment variable carrying the nested `hnl run <name>` depth.
pub const DEPTH_ENV: &str = "HANDL_RUN_DEPTH";

/// Maximum nested `hnl run <name>` depth — runaway/self recursion
/// becomes a diagnostic instead of a stack of subprocesses.
pub const MAX_RUN_DEPTH: u32 = 8;

/// Environment variable overriding the `hnl` binary location (the
/// fallback when `current_exe()` is not the `hnl` host).
pub const HNL_ENV: &str = "HANDL_HNL";

/// The captured result of running the segments of one script/task.
#[derive(Debug, Default)]
pub struct ExecOutcome {
    /// Combined exit code — the first nonzero segment's, or 0.
    pub code: i32,
    /// Everything the segments wrote to stdout, concatenated.
    pub stdout: Vec<u8>,
    /// Everything the segments wrote to stderr, concatenated.
    pub stderr: Vec<u8>,
}

/// Whether `path` names the `hnl` host binary (`hnl`/`hnl.exe`).
pub fn is_hnl_host(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("hnl") | Some("hnl.exe")
    )
}

/// Resolve an `hnl` argv[0] to the program to spawn.
///
/// `current_exe` wins only when it *is* the `hnl` host; otherwise
/// `$HANDL_HNL` (when set) then plain `"hnl"` (PATH lookup by
/// [`Command`]). Pure — the env-reading wrapper is
/// [`resolve_hnl_program`].
pub fn resolve_hnl_argv0(current_exe: &Path, handl_hnl: Option<&OsStr>) -> OsString {
    if is_hnl_host(current_exe) {
        return current_exe.as_os_str().to_os_string();
    }
    if let Some(p) = handl_hnl {
        return p.to_os_string();
    }
    OsString::from("hnl")
}

/// [`resolve_hnl_argv0`] reading `current_exe()`/`$HANDL_HNL` itself.
pub fn resolve_hnl_program() -> Result<OsString, RunError> {
    let exe = std::env::current_exe().map_err(|e| {
        RunError::io(format!(
            "cannot determine current_exe for `hnl` argv[0] resolution: {e}"
        ))
    })?;
    Ok(resolve_hnl_argv0(
        &exe,
        std::env::var_os(HNL_ENV).as_deref(),
    ))
}

/// The current `HANDL_RUN_DEPTH` (0 when unset or unparsable).
pub fn current_depth() -> u32 {
    std::env::var(DEPTH_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Whether `args` — the tokens after `hnl run` — name a *script* (as
/// opposed to a `.hnl` file) under `cwd`.
///
/// Mirrors the runner's name-vs-file precedence approximately: runner
/// flags (`--plan`/`--cache`/`--no-cache`/`--json`/`--`) are skipped,
/// file-exec flags (`--engine`/`--name`/`--kit`) and their values make it
/// a file invocation, and the first remaining token decides — `.hnl`
/// suffix or an existing file under `cwd` means file execution.
/// Heuristic by design: the spawned `hnl` re-dispatches authoritatively;
/// this only decides whether `HANDL_RUN_DEPTH` increments.
pub fn is_script_invocation(args: &[String], cwd: &Path) -> bool {
    let mut i = 0;
    while i < args.len() {
        let t = &args[i];
        match t.as_str() {
            "--plan" | "--cache" | "--no-cache" | "--json" | "--" => i += 1,
            "--engine" | "--name" | "--kit" => i += 2,
            _ if t.starts_with('-') => return false,
            _ => {
                return !(t.ends_with(".hnl") || cwd.join(t).is_file());
            }
        }
    }
    false
}

/// Extra env vars to set on a spawned command, given its (pre-resolution)
/// argv and working directory.
///
/// Returns `(HANDL_RUN_DEPTH, depth+1)` when the command is a nested
/// `hnl run <script-name>` spawn; empty otherwise. `depth` is the
/// *current* run depth ([`current_depth`] or the caller's override).
pub fn depth_env_for(argv: &Argv, cwd: &Path, depth: u32) -> Vec<(OsString, OsString)> {
    if argv.len() < 2 || argv[0] != "hnl" || argv[1] != "run" {
        return Vec::new();
    }
    if is_script_invocation(&argv[2..], cwd) {
        vec![(
            OsString::from(DEPTH_ENV),
            OsString::from((depth + 1).to_string()),
        )]
    } else {
        Vec::new()
    }
}

/// Resolve argv[0] (`hnl` → host binary) without executing.
fn resolve_argv0(argv: &Argv) -> Result<(OsString, Vec<String>), RunError> {
    let mut argv = argv.to_vec();
    if argv.is_empty() {
        return Err(RunError::io("empty argv segment"));
    }
    if argv[0] == "hnl" || argv[0] == "hnl.exe" {
        argv[0] = resolve_hnl_program()?.to_string_lossy().into_owned();
    }
    let program = OsString::from(&argv[0]);
    argv.remove(0);
    Ok((program, argv))
}

/// Execute `segments` sequentially in `cwd`, capturing combined
/// stdout/stderr. Stops at the first nonzero exit.
///
/// `depth` is the current `HANDL_RUN_DEPTH` (see [`current_depth`]) —
/// used only to compute the increment for nested `hnl run <name>`
/// spawns.
pub fn exec_segments(segments: &[Argv], cwd: &Path, depth: u32) -> Result<ExecOutcome, RunError> {
    let mut out = ExecOutcome::default();
    for argv in segments {
        if argv.is_empty() {
            continue;
        }
        let (program, args) = resolve_argv0(argv)?;
        let mut cmd = Command::new(&program);
        cmd.args(&args).current_dir(cwd);
        for (k, v) in depth_env_for(argv, cwd, depth) {
            cmd.env(k, v);
        }
        let output = cmd.output().map_err(|e| RunError::Spawn {
            argv0: argv[0].clone(),
            source: e,
        })?;
        out.stdout.extend_from_slice(&output.stdout);
        out.stderr.extend_from_slice(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        if code != 0 {
            out.code = code;
            return Ok(out);
        }
    }
    Ok(out)
}

/// The `cwd` a task runs in: `cwd` (relative to the package root) or the
/// package root itself.
///
/// # Errors
///
/// [`RunError::BadCwd`] when the resolved directory does not exist.
pub fn task_cwd(pkg_dir: &Path, script: &str, cwd: Option<&str>) -> Result<PathBuf, RunError> {
    let dir = match cwd {
        Some(rel) => pkg_dir.join(rel),
        None => pkg_dir.to_path_buf(),
    };
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(RunError::BadCwd {
            script: script.to_string(),
            cwd: cwd.unwrap_or(".").to_string(),
            base: pkg_dir.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hnl_argv0_prefers_hnl_host_exe() {
        let exe = Path::new("/usr/local/bin/hnl");
        assert_eq!(
            resolve_hnl_argv0(exe, Some(OsStr::new("/other/hnl"))),
            OsString::from("/usr/local/bin/hnl")
        );
        // `hnl.exe` counts as the host name too (Windows builds).
        assert!(is_hnl_host(Path::new("/opt/hnl/hnl.exe")));
        assert!(is_hnl_host(Path::new("hnl")));
    }

    #[test]
    fn hnl_argv0_falls_back_to_env_then_path() {
        // A test binary is never `hnl` → $HANDL_HNL wins.
        let exe = Path::new("/target/debug/deps/handl_run-abc123");
        assert_eq!(
            resolve_hnl_argv0(exe, Some(OsStr::new("/opt/hnl-dev"))),
            OsString::from("/opt/hnl-dev")
        );
        // Neither → bare "hnl" for PATH resolution.
        assert_eq!(resolve_hnl_argv0(exe, None), OsString::from("hnl"));
        assert!(!is_hnl_host(Path::new("/usr/bin/not-hnl")));
        assert!(!is_hnl_host(Path::new("hnl-wrapper")));
    }

    #[test]
    fn script_invocation_detection() {
        let cwd = Path::new("/tmp");
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(is_script_invocation(&s(&["gate"]), cwd));
        assert!(is_script_invocation(&s(&["--json", "gate", "--flag"]), cwd));
        assert!(is_script_invocation(&s(&["--", "gate"]), cwd));
        assert!(!is_script_invocation(&s(&["file.hnl"]), cwd));
        assert!(!is_script_invocation(
            &s(&["--engine", "x", "file.hnl"]),
            cwd
        ));
        assert!(!is_script_invocation(&s(&[]), cwd));
        // An existing file under cwd is file-exec even without .hnl.
        let dir = std::env::temp_dir().join(format!("hnl-inv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plain"), "").unwrap();
        assert!(!is_script_invocation(&s(&["plain"]), &dir));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn depth_env_only_for_script_runs() {
        let cwd = Path::new("/tmp");
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        // `hnl run gate` → depth+1.
        let env = depth_env_for(&s(&["hnl", "run", "gate"]), cwd, 3);
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].1, OsString::from("4"));
        // `hnl run file.hnl` → no increment (file exec).
        assert!(depth_env_for(&s(&["hnl", "run", "x.hnl"]), cwd, 3).is_empty());
        // `hnl eval …` / other argv[0] → nothing.
        assert!(depth_env_for(&s(&["hnl", "eval", "1+1"]), cwd, 3).is_empty());
        assert!(depth_env_for(&s(&["cargo", "test"]), cwd, 3).is_empty());
    }
}
