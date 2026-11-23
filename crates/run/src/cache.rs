//! The fingerprinted task cache — `.handl/cache/run/<key>/` inside the
//! package (P-003).
//!
//! ```text
//! .handl/cache/run/<key>/
//!   meta.json      { script, args, code, toolchain, env, inputs }
//!   stdout  stderr
//!   outputs/…      blob tree of declared outputs at run time
//! ```
//!
//! **Key** = sha256 over the normalized script definition + appended CLI
//! args. The *contents* half of the spec's key list is realized as the
//! per-input manifest comparison in [`EntryMeta::inputs`]: on lookup the
//! current input set is diffed against the recorded manifest (with an
//! mtime/size pre-filter that skips re-hashing unchanged files), which
//! is what lets a miss *name* the culprit file — `cache miss: input
//! 'src/foo.hnl' modified`. Env vars and the toolchain id are likewise
//! recorded in `meta.json` and compared, producing `env 'X' changed` /
//! `toolchain changed` / `args changed` reasons. (Deviation note: the
//! proposal lists all four components as hashed *into* the key; hashing
//! inputs/env/args into the directory name would make every change a
//! bare "no cache entry" and the named miss reasons unreachable. The
//! observable contract — hit/miss verdicts, named culprits, drift
//! refusal — is identical.)
//!
//! **Hit** restores each declared `outputs` blob only when the current
//! file is missing; an existing file that differs is *drift* → miss,
//! never a silent clobber. Then recorded streams replay and the
//! recorded exit code returns. **Miss** runs normally; on exit 0 the
//! entry is snapshotted via tmp-dir + rename. Failures are never
//! cached; a corrupt entry is treated as a miss and overwritten.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::RunError;
use crate::exec::ExecOutcome;
use crate::pkg::PkgScripts;
use crate::script::{Script, Task, canonical_task};

/// `.handl/cache/run` under the package root.
pub fn cache_root(pkg_dir: &Path) -> PathBuf {
    pkg_dir.join(".handl").join("cache").join("run")
}

/// The toolchain identity folded into every cache entry: the `hnl` host
/// version **and** the `hnl-ext-eval` version — the code that actually
/// runs `.hnl` files (both `CARGO_PKG_VERSION`s; the runner crate
/// cannot see them, so the consumer passes them in — P-003).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toolchain {
    /// `hnl` host version string.
    pub hnl: String,
    /// `hnl-ext-eval` extension version string.
    pub ext_eval: String,
}

impl Toolchain {
    /// Detect the toolchain from the environment: `HANDL_HNL_VERSION` /
    /// `HANDL_EXT_EVAL_VERSION` when the host exports them, `"unknown"`
    /// otherwise. `ext-eval` overrides with its own `CARGO_PKG_VERSION`
    /// when it wires the runner in.
    pub fn detect() -> Self {
        Toolchain {
            hnl: std::env::var("HANDL_HNL_VERSION").unwrap_or_else(|_| "unknown".into()),
            ext_eval: std::env::var("HANDL_EXT_EVAL_VERSION").unwrap_or_else(|_| "unknown".into()),
        }
    }
}

/// Why a cache lookup missed — the Vite-Task-style culprit naming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissReason {
    /// No entry exists for this script+args.
    NoEntry,
    /// `meta.json` exists but does not parse — treated as a miss and
    /// overwritten on the next successful run.
    CorruptEntry,
    /// Recorded args differ (defensive — args are already in the key).
    ArgsChanged,
    /// A fingerprinted env var changed value (or appeared/disappeared).
    EnvChanged(String),
    /// The hnl host or ext-eval version changed.
    ToolchainChanged,
    /// A file matching `inputs` is new since the recorded run.
    InputAdded(String),
    /// A recorded input no longer matches the globs.
    InputRemoved(String),
    /// A recorded input's contents changed.
    InputModified(String),
    /// A cached output exists on disk with different bytes.
    OutputDrifted(String),
}

impl std::fmt::Display for MissReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MissReason::NoEntry => write!(f, "no cache entry"),
            MissReason::CorruptEntry => write!(f, "cache entry unreadable"),
            MissReason::ArgsChanged => write!(f, "args changed"),
            MissReason::EnvChanged(name) => write!(f, "env '{name}' changed"),
            MissReason::ToolchainChanged => write!(f, "toolchain changed"),
            MissReason::InputAdded(p) => write!(f, "input '{p}' added"),
            MissReason::InputRemoved(p) => write!(f, "input '{p}' removed"),
            MissReason::InputModified(p) => write!(f, "input '{p}' modified"),
            MissReason::OutputDrifted(p) => write!(f, "output '{p}' drifted"),
        }
    }
}

/// The outcome of a cache lookup for one task invocation.
#[derive(Debug)]
pub enum CacheCheck {
    /// Inputs/env/toolchain/outputs all match — replay the entry.
    Hit {
        /// The entry directory (`.handl/cache/run/<key>`).
        entry: PathBuf,
        /// The recorded metadata (streams' exit code, env, inputs).
        meta: EntryMeta,
    },
    /// Anything else — run the task, naming the culprit.
    Miss(MissReason),
}

/// `meta.json` — everything needed to validate a hit and replay it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryMeta {
    /// Schema version.
    pub version: u32,
    /// Script name.
    pub script: String,
    /// CLI args appended to the script's last command.
    pub args: Vec<String>,
    /// Recorded exit code (always 0 — failures are never cached).
    pub code: i32,
    /// Toolchain id at run time.
    pub toolchain: Toolchain,
    /// Fingerprinted env vars: name → value (`None` = unset).
    pub env: BTreeMap<String, Option<String>>,
    /// Per-input manifest: display key → stat+content record. This is
    /// what lets a miss name the changed file.
    pub inputs: BTreeMap<String, InputRecord>,
}

/// One input's fingerprint: mtime+size pre-filter plus content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputRecord {
    /// `mtime` seconds since UNIX_EPOCH.
    pub mtime_secs: u64,
    /// `mtime` nanoseconds.
    pub mtime_nanos: u32,
    /// File size in bytes.
    pub size: u64,
    /// sha256 of the file contents, hex.
    pub sha256: String,
}

/// The cache key for one (script, args) pair — sha256 hex of the
/// normalized definition. See the module docs for the deviation note on
/// what is *not* hashed into the key.
pub fn entry_key(script_name: &str, task: &Task, args: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(b"handl-run/run-cache/v1\n");
    h.update(format!("name={script_name}\n"));
    h.update(format!("args={}\n", args.len()));
    for a in args {
        h.update(a.as_bytes());
        h.update(b"\x1f");
    }
    h.update(b"def=");
    h.update(canonical_task(task).as_bytes());
    hex::encode(h.finalize())
}

/// Display key for a path relative to the package root: `src/a.hnl`
/// inside the package, `@/abs/path` outside it (declared inputs/outputs
/// may legitimately point at `../` siblings — P-003's dep-tree gap).
fn display_key(pkg_dir: &Path, path: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(pkg_dir) {
        rel.to_string_lossy().replace('\\', "/")
    } else {
        format!("@{}", path.to_string_lossy())
    }
}

/// Absolute path back from a [`display_key`].
fn key_to_path(pkg_dir: &Path, key: &str) -> PathBuf {
    if let Some(abs) = key.strip_prefix('@') {
        PathBuf::from(abs)
    } else {
        pkg_dir.join(key)
    }
}

/// Expand `globs` (relative to `pkg_dir`) into sorted display-key →
/// absolute-path pairs, files only.
fn expand_globs(pkg_dir: &Path, globs: &[String]) -> Result<BTreeMap<String, PathBuf>, RunError> {
    let base = std::fs::canonicalize(pkg_dir).unwrap_or_else(|_| pkg_dir.to_path_buf());
    let mut out = BTreeMap::new();
    for glob in globs {
        let pattern = base.join(glob).to_string_lossy().into_owned();
        let paths = glob::glob(&pattern).map_err(|e| RunError::Glob {
            pattern: pattern.clone(),
            source: e,
        })?;
        for entry in paths {
            let entry = entry.map_err(|e| RunError::io(format!("glob `{pattern}`: {e}")))?;
            let Ok(abs) = std::fs::canonicalize(&entry) else {
                continue;
            };
            if !abs.is_file() {
                continue;
            }
            out.insert(display_key(&base, &abs), abs);
        }
    }
    Ok(out)
}

/// The default input set when `inputs` is undeclared: the package's own
/// module sources (every existing [`pkg_module_candidates`] file) plus
/// `pkg.hnl` itself — the documented v1 default (dep trees and non-`.hnl`
/// reads are *not* covered; declare them in `inputs:`).
fn default_input_globs(pkg: &PkgScripts) -> Vec<String> {
    use handl_platform::config::pkg_module_candidates;
    let mut globs = Vec::new();
    for module in &pkg.modules {
        for cand in pkg_module_candidates(&pkg.modules, pkg.entry.as_deref(), module) {
            if pkg.dir.join(&cand).is_file() {
                globs.push(cand);
            }
        }
    }
    globs.push("pkg.hnl".to_string());
    globs
}

/// The current input set for a task: display key → absolute path.
fn input_files(pkg: &PkgScripts, task: &Task) -> Result<BTreeMap<String, PathBuf>, RunError> {
    let globs = match &task.inputs {
        Some(globs) => globs.clone(),
        None => default_input_globs(pkg),
    };
    expand_globs(&pkg.dir, &globs)
}

/// mtime+size of `path` (the pre-filter half of [`InputRecord`]).
fn stat_part(path: &Path) -> Option<(u64, u32, u64)> {
    let md = fs::metadata(path).ok()?;
    let mtime = md.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((mtime.as_secs(), mtime.subsec_nanos(), md.len()))
}

/// sha256 hex of a file's contents.
fn hash_file(path: &Path) -> Result<String, RunError> {
    let mut f = fs::File::open(path)
        .map_err(|e| RunError::io(format!("cannot read input {}: {e}", path.display())))?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| RunError::io(format!("cannot hash {}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Full record (stat + content hash) for one input.
fn record_input(path: &Path) -> Result<InputRecord, RunError> {
    let (mtime_secs, mtime_nanos, size) =
        stat_part(path).ok_or_else(|| RunError::io(format!("cannot stat {}", path.display())))?;
    Ok(InputRecord {
        mtime_secs,
        mtime_nanos,
        size,
        sha256: hash_file(path)?,
    })
}

/// The fingerprinted env manifest for a task: declared names →
/// current value (`None` when unset — unset vs set both hash).
fn env_manifest(task: &Task) -> BTreeMap<String, Option<String>> {
    let mut names: Vec<&String> = task.env.iter().collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|n| {
            let v = std::env::var(n).ok();
            (n.clone(), v)
        })
        .collect()
}

/// Look up the cache entry for `(pkg, script, task, args)`.
///
/// `script_name`/`args` only feed the key; `task` supplies the def,
/// inputs, outputs, env, and cwd that are validated.
///
/// # Errors
///
/// [`RunError::Glob`] / [`RunError::Io`] on unreadable inputs — a cache
/// *check* failing closed is correct: the caller surfaces the error
/// rather than silently running uncached.
pub fn check(
    pkg: &PkgScripts,
    script_name: &str,
    task: &Task,
    args: &[String],
    toolchain: &Toolchain,
) -> Result<CacheCheck, RunError> {
    let key = entry_key(script_name, task, args);
    let entry = cache_root(&pkg.dir).join(&key);
    let meta_path = entry.join("meta.json");
    if !meta_path.is_file() {
        return Ok(CacheCheck::Miss(MissReason::NoEntry));
    }
    let meta: EntryMeta = match fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
    {
        Some(m) => m,
        None => return Ok(CacheCheck::Miss(MissReason::CorruptEntry)),
    };

    // args → env → toolchain → inputs → output drift, in spec order.
    if meta.args != args {
        return Ok(CacheCheck::Miss(MissReason::ArgsChanged));
    }
    let current_env = env_manifest(task);
    for (name, recorded) in &meta.env {
        match current_env.get(name) {
            Some(v) if v == recorded => {}
            _ => return Ok(CacheCheck::Miss(MissReason::EnvChanged(name.clone()))),
        }
    }
    // Names present now but absent from the manifest also count.
    for name in current_env.keys() {
        if !meta.env.contains_key(name) {
            return Ok(CacheCheck::Miss(MissReason::EnvChanged(name.clone())));
        }
    }
    if meta.toolchain != *toolchain {
        return Ok(CacheCheck::Miss(MissReason::ToolchainChanged));
    }

    // Inputs: union of current and recorded keys, sorted — first
    // culprit wins, deterministically.
    let current = input_files(pkg, task)?;
    let mut keys: Vec<&String> = current.keys().chain(meta.inputs.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        match (current.get(key), meta.inputs.get(key)) {
            (Some(_), None) => {
                return Ok(CacheCheck::Miss(MissReason::InputAdded(key.clone())));
            }
            (None, Some(_)) => {
                return Ok(CacheCheck::Miss(MissReason::InputRemoved(key.clone())));
            }
            (Some(path), Some(recorded)) => {
                // mtime/size pre-filter: unchanged stat ⇒ unchanged
                // content, skip the re-hash entirely.
                let unchanged = stat_part(path).is_some_and(|(s, n, z)| {
                    s == recorded.mtime_secs && n == recorded.mtime_nanos && z == recorded.size
                });
                if unchanged {
                    continue;
                }
                let now = record_input(path)?;
                if now.sha256 != recorded.sha256 {
                    return Ok(CacheCheck::Miss(MissReason::InputModified(key.clone())));
                }
            }
            (None, None) => {}
        }
    }

    // Output drift: every cached blob must be missing or byte-identical
    // on disk — a differing file is user work, never clobbered.
    let outputs_dir = entry.join("outputs");
    if outputs_dir.is_dir() {
        for blob in walk_files(&outputs_dir)? {
            let rel = blob
                .strip_prefix(&outputs_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let current_path = key_to_path(&pkg.dir, &rel);
            if current_path.exists() {
                let same = fs::read(&blob)
                    .ok()
                    .zip(fs::read(&current_path).ok())
                    .is_some_and(|(a, b)| a == b);
                if !same {
                    return Ok(CacheCheck::Miss(MissReason::OutputDrifted(rel)));
                }
            }
        }
    }

    Ok(CacheCheck::Hit { entry, meta })
}

/// Restore cached outputs that are *missing* on disk. Drift was already
/// ruled out by [`check`]; an existing file is left untouched.
///
/// Returns the display keys restored (for reporting).
pub fn restore_missing_outputs(pkg: &PkgScripts, entry: &Path) -> Result<Vec<String>, RunError> {
    let outputs_dir = entry.join("outputs");
    let mut restored = Vec::new();
    if !outputs_dir.is_dir() {
        return Ok(restored);
    }
    for blob in walk_files(&outputs_dir)? {
        let rel = blob
            .strip_prefix(&outputs_dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let current = key_to_path(&pkg.dir, &rel);
        if current.exists() {
            continue;
        }
        if let Some(parent) = current.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| RunError::io(format!("cannot create {}: {e}", parent.display())))?;
        }
        fs::copy(&blob, &current)
            .map_err(|e| RunError::io(format!("cannot restore {}: {e}", current.display())))?;
        restored.push(rel);
    }
    Ok(restored)
}

/// Snapshot a successful run into the cache entry — tmp dir + rename so
/// two concurrent `hnl run`s on the same key never interleave a
/// half-written entry. A pre-existing entry is removed first (rename
/// cannot replace a non-empty dir); the worst case for a concurrent
/// reader is a miss, never a corrupt hit.
///
/// Streams are always stored; declared `outputs` blobs are stored only
/// when the task declares them.
///
/// # Errors
///
/// Filesystem failures — the caller treats store failure as a warning,
/// never a run failure.
pub fn store(
    pkg: &PkgScripts,
    script: &Script,
    task: &Task,
    args: &[String],
    toolchain: &Toolchain,
    outcome: &ExecOutcome,
) -> Result<(), RunError> {
    debug_assert_eq!(outcome.code, 0, "failures are never cached");
    let key = entry_key(&script.name, task, args);
    let root = cache_root(&pkg.dir);
    fs::create_dir_all(&root)
        .map_err(|e| RunError::io(format!("cannot create {}: {e}", root.display())))?;

    // Fresh input manifest — every file hashed (store is rare; the
    // pre-filter only pays on lookup).
    let mut inputs = BTreeMap::new();
    for (k, path) in input_files(pkg, task)? {
        inputs.insert(k, record_input(&path)?);
    }
    let meta = EntryMeta {
        version: 1,
        script: script.name.clone(),
        args: args.to_vec(),
        code: outcome.code,
        toolchain: toolchain.clone(),
        env: env_manifest(task),
        inputs,
    };

    // Build in a private tmp dir, then publish by rename.
    let tmp = root.join(format!(
        ".{key}.tmp-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    if tmp.exists() {
        fs::remove_dir_all(&tmp).ok();
    }
    fs::create_dir_all(&tmp)
        .map_err(|e| RunError::io(format!("cannot create {}: {e}", tmp.display())))?;
    let result = (|| -> Result<(), RunError> {
        fs::write(
            tmp.join("meta.json"),
            serde_json::to_string_pretty(&meta)
                .map_err(|e| RunError::io(format!("cannot serialize meta.json: {e}")))?,
        )
        .map_err(|e| RunError::io(format!("cannot write meta.json: {e}")))?;
        fs::write(tmp.join("stdout"), &outcome.stdout)
            .map_err(|e| RunError::io(format!("cannot write stdout: {e}")))?;
        fs::write(tmp.join("stderr"), &outcome.stderr)
            .map_err(|e| RunError::io(format!("cannot write stderr: {e}")))?;

        if let Some(globs) = &task.outputs
            && !globs.is_empty()
        {
            for (key, abs) in expand_globs(&pkg.dir, globs)? {
                let dest = tmp.join("outputs").join(&key);
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        RunError::io(format!("cannot create {}: {e}", parent.display()))
                    })?;
                }
                fs::copy(&abs, &dest).map_err(|e| {
                    RunError::io(format!("cannot snapshot output {}: {e}", abs.display()))
                })?;
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        fs::remove_dir_all(&tmp).ok();
        return Err(e);
    }

    let entry = root.join(&key);
    if entry.exists() {
        fs::remove_dir_all(&entry)
            .map_err(|e| RunError::io(format!("cannot replace {}: {e}", entry.display())))?;
    }
    fs::rename(&tmp, &entry).map_err(|e| {
        fs::remove_dir_all(&tmp).ok();
        RunError::io(format!(
            "cannot publish cache entry {}: {e}",
            entry.display()
        ))
    })?;
    Ok(())
}

/// Nanosecond-ish suffix for tmp dir uniqueness within a process.
fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// All regular files under `dir`, recursively, sorted.
fn walk_files(dir: &Path) -> Result<Vec<PathBuf>, RunError> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let rd = fs::read_dir(&d)
            .map_err(|e| RunError::io(format!("cannot read {}: {e}", d.display())))?;
        for entry in rd {
            let entry =
                entry.map_err(|e| RunError::io(format!("cannot read {}: {e}", d.display())))?;
            let path = entry.path();
            let ft = entry
                .file_type()
                .map_err(|e| RunError::io(format!("cannot stat {}: {e}", path.display())))?;
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Recorded streams of a hit, for replay.
pub fn read_streams(entry: &Path) -> (Vec<u8>, Vec<u8>) {
    (
        fs::read(entry.join("stdout")).unwrap_or_default(),
        fs::read(entry.join("stderr")).unwrap_or_default(),
    )
}
