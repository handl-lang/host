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
//! ## Input scope (P-003 v2 — strict by default)
//!
//! What counts as an *input* is decided per task:
//!
//! - **`inputs:` declared** — exactly those globs, nothing else. This
//!   is the narrowing escape hatch for big repos *and* the only way to
//!   fingerprint gitignored files (e.g. `generated/**`).
//! - **`inputs:` absent, package inside a git work tree** — every file
//!   `git ls-files --cached --others --exclude-standard` reports under
//!   the package root (tracked *and* untracked-but-not-ignored), minus
//!   `.handl/cache/`, **plus every `pkg.hnl` `deps` tree's files the
//!   same way** (sibling `lib-<name>` checkouts, transitively). An
//!   undeclared read — `bash ci.sh`, `Cargo.toml`, a sibling repo —
//!   can no longer go stale silently.
//! - **`inputs:` absent, no git work tree** — falls back to the v1
//!   default (module sources + `pkg.hnl`) and emits a one-shot
//!   `warning:` notice; dep trees are still covered.
//!
//! In the non-declared scopes the task's declared `outputs` globs are
//! subtracted from the input set: an output is produced, not read, and
//! leaving it in would turn every regenerate into a self-inflicted
//! miss (and break restore-missing-outputs for unignored files).
//!
//! Boundaries, honestly: untracked *ignored* files (`target/`,
//! `node_modules`, gitignored `generated/`) are not fingerprinted —
//! declare them in `inputs:`; `.handl.toml` `[dependencies]`/`[patch]`
//! dirs and package-cache git deps are not covered (only `pkg.hnl`
//! `deps` → sibling `lib-<name>`); nested git repos/submodules appear
//! only as their outer-repo listing.
//!
//! **Hit** restores each declared `outputs` blob only when the current
//! file is missing; an existing file that differs is *drift* → miss,
//! never a silent clobber. Then recorded streams replay and the
//! recorded exit code returns. **Miss** runs normally; on exit 0 the
//! entry is snapshotted via tmp-dir + rename. Failures are never
//! cached; a corrupt entry is treated as a miss and overwritten.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use handl_platform::config::resolve_pkg_dependencies;

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
    /// The entry was written under a different cache schema — v2 keys
    /// already keep v1 entries apart, this guards future bumps.
    SchemaChanged,
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
            MissReason::SchemaChanged => write!(f, "cache schema changed"),
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

/// Cache-entry schema version — bumped when input-scope semantics or
/// the meta layout change. v2 = the git-`ls-files` default input scope
/// (P-003 v2); v1 entries live under `…/v1` keys and simply never
/// match, so no silent reuse of a stale-semantics hit is possible.
pub const CACHE_SCHEMA_VERSION: u32 = 2;

/// The cache key for one (script, args) pair — sha256 hex of the
/// normalized definition. See the module docs for the deviation note on
/// what is *not* hashed into the key.
pub fn entry_key(script_name: &str, task: &Task, args: &[String]) -> String {
    let mut h = Sha256::new();
    h.update(format!("handl-run/run-cache/v{CACHE_SCHEMA_VERSION}\n").as_bytes());
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

/// The v1 default input set — module sources (every existing
/// [`pkg_module_candidates`] file) plus `pkg.hnl` itself. Under v2 this
/// is only the *fallback* for directories outside a git work tree (and
/// for non-git dep trees); the git scope covers everything tracked.
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

/// The resolved input set for one task plus runner notices.
struct InputScan {
    /// Display key → absolute path (see [`display_key`]).
    files: BTreeMap<String, PathBuf>,
    /// One-shot warnings (currently only the non-git fallback).
    notices: Vec<String>,
}

/// `git ls-files -z --cached --others --exclude-standard` under `dir`,
/// as absolute paths — tracked *and* untracked-but-not-ignored files,
/// minus `.handl/cache/` (generated state that must never be an
/// input). `None` when `dir` is not inside a git work tree or `git`
/// is unavailable — the caller falls back.
fn git_files(dir: &Path) -> Option<Vec<PathBuf>> {
    let dir = std::fs::canonicalize(dir).ok()?;
    let out = Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut files = Vec::new();
    for rec in out.stdout.split(|b| *b == 0) {
        if rec.is_empty() {
            continue;
        }
        // `-z` output is unquoted; non-UTF-8 names degrade to lossy
        // (the display key is a String either way).
        let rel = String::from_utf8_lossy(rec).replace('\\', "/");
        if rel == ".handl/cache" || rel.starts_with(".handl/cache/") {
            continue;
        }
        let abs = dir.join(&rel);
        // `--cached` lists tracked files deleted from the worktree —
        // skip them so they surface as `InputRemoved` in the manifest
        // diff instead of a stat error.
        if abs.is_file() {
            files.push(abs);
        }
    }
    Some(files)
}

/// Whether `dir` sits inside a git work tree — memoized per directory
/// (work-tree-ness does not change within a process; the file *list*
/// deliberately is not memoized).
fn in_git_work_tree(dir: &Path) -> bool {
    static PROBE: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let probe = PROBE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if let Ok(map) = probe.lock()
        && let Some(v) = map.get(&key)
    {
        return *v;
    }
    let inside = Command::new("git")
        .arg("-C")
        .arg(&key)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success() && o.stdout.starts_with(b"true"))
        .unwrap_or(false);
    if let Ok(mut map) = probe.lock() {
        map.insert(key, inside);
    }
    inside
}

/// The one-shot fallback warning — at most one notice per directory
/// per process, no matter how many tasks scan it.
fn fallback_notice(dir: &Path, role: &str) -> Option<String> {
    static NOTICED: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();
    let noticed = NOTICED.get_or_init(|| Mutex::new(BTreeSet::new()));
    let key = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let mut guard = noticed.lock().ok()?;
    if !guard.insert(key.clone()) {
        return None;
    }
    Some(format!(
        "warning: {role} {} is not inside a git work tree (or git is unavailable) — \
         cache inputs cover only pkg.hnl + module sources there; undeclared file reads \
         can go stale (declare `inputs:` or run inside git)",
        key.display()
    ))
}

/// `pkg.hnl` `deps` → sibling `lib-<name>` checkouts, transitively —
/// the same convention [`resolve_pkg_dependencies`] applies, closed
/// breadth-first over each dep's own `pkg.hnl` (visited-set bounded, so
/// dep cycles terminate). Only directories that exist are returned,
/// canonicalized and deduplicated.
fn dep_dirs(pkg_dir: &Path) -> Vec<PathBuf> {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let root = canon(pkg_dir);
    let mut seen: HashSet<PathBuf> = HashSet::from([root.clone()]);
    let mut queue: VecDeque<PathBuf> = VecDeque::from([root]);
    let mut out = Vec::new();
    while let Some(dir) = queue.pop_front() {
        let Ok(deps) = resolve_pkg_dependencies(&dir) else {
            continue;
        };
        for (_name, dep) in deps {
            if !dep.is_dir() {
                continue;
            }
            let dep = canon(&dep);
            if seen.insert(dep.clone()) {
                out.push(dep.clone());
                queue.push_back(dep);
            }
        }
    }
    out
}

/// Insert `abs` under its display key, and — when `pkg.hnl` exists at
/// `root` — make sure the manifest itself is always an input (it can
/// be gitignored in exotic layouts; the def hash does not cover
/// non-script keys like `modules`/`deps`).
fn insert_root_files(
    files: &mut BTreeMap<String, PathBuf>,
    pkg_dir: &Path,
    root: &Path,
    paths: impl IntoIterator<Item = PathBuf>,
) {
    for abs in paths {
        files.insert(display_key(pkg_dir, &abs), abs);
    }
    let manifest = root.join("pkg.hnl");
    if manifest.is_file() {
        files.insert(display_key(pkg_dir, &manifest), manifest);
    }
}

/// The current input set for a task: display key → absolute path,
/// plus one-shot notices. See the module docs for the v2 scope rules.
fn scan_inputs(pkg: &PkgScripts, task: &Task) -> Result<InputScan, RunError> {
    // `inputs:` declared → exactly those globs. The narrowing escape
    // hatch for big repos, and the only way to fingerprint ignored
    // files (e.g. `generated/**`).
    if let Some(globs) = &task.inputs {
        return Ok(InputScan {
            files: expand_globs(&pkg.dir, globs)?,
            notices: Vec::new(),
        });
    }

    let pkg_base = std::fs::canonicalize(&pkg.dir).unwrap_or_else(|_| pkg.dir.clone());
    let mut files = BTreeMap::new();
    let mut notices = Vec::new();

    // The package's own files.
    match git_files(&pkg_base) {
        Some(own) => insert_root_files(&mut files, &pkg_base, &pkg_base, own),
        None => {
            if let Some(n) = fallback_notice(&pkg_base, "package root") {
                notices.push(n);
            }
            let globs = default_input_globs(pkg);
            for (k, p) in expand_globs(&pkg.dir, &globs)? {
                files.insert(k, p);
            }
        }
    }

    // Dep trees: `pkg.hnl` `deps` → sibling `lib-<name>` checkouts,
    // transitively. Each dep is fingerprinted by the same rule —
    // git-tracked files when it is a work tree, else its own
    // `pkg.hnl` + module sources. Keys outside the package are `@/abs`.
    for dep in dep_dirs(&pkg_base) {
        match git_files(&dep) {
            Some(dep_files) => insert_root_files(&mut files, &pkg_base, &dep, dep_files),
            None => {
                if let Some(n) = fallback_notice(&dep, "dependency") {
                    notices.push(n);
                }
                if let Some(dep_pkg) = PkgScripts::load(&dep) {
                    let globs = default_input_globs(&dep_pkg);
                    for (_k, p) in expand_globs(&dep, &globs)? {
                        files.insert(display_key(&pkg_base, &p), p);
                    }
                    let manifest = dep.join("pkg.hnl");
                    if manifest.is_file() {
                        files.insert(display_key(&pkg_base, &manifest), manifest);
                    }
                }
            }
        }
    }

    // Declared outputs are produced, not read — subtract them so an
    // unignored output file cannot self-bust the cache or break
    // restore-missing-outputs. (`inputs:`-declared scope is exempt:
    // there the user named the exact set.)
    if let Some(globs) = &task.outputs
        && !globs.is_empty()
    {
        for (_k, p) in expand_globs(&pkg.dir, globs)? {
            files.remove(&display_key(&pkg_base, &p));
        }
    }

    Ok(InputScan { files, notices })
}

/// Early fallback notice for `check_noticed`: when `inputs:` is
/// undeclared and the package root is not a git work tree, the notice
/// is emitted even on misses that return before the input scan
/// (NoEntry/Corrupt/Args/Env/Toolchain). Dep-level notices still
/// surface from [`scan_inputs`] on the input stage.
fn early_notices(pkg: &PkgScripts, task: &Task) -> Vec<String> {
    if task.inputs.is_some() {
        return Vec::new();
    }
    let base = std::fs::canonicalize(&pkg.dir).unwrap_or_else(|_| pkg.dir.clone());
    if in_git_work_tree(&base) {
        return Vec::new();
    }
    fallback_notice(&base, "package root").into_iter().collect()
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

/// Look up the cache entry for `(pkg, script, task, args)` — the
/// notice-free half of [`check_noticed`].
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
    Ok(check_noticed(pkg, script_name, task, args, toolchain)?.0)
}

/// [`check`] plus one-shot notices about how the input set was
/// resolved — currently the non-git fallback warning (P-003 v2). The
/// `Runner` merges these into the run/plan report.
///
/// # Errors
///
/// Same as [`check`].
pub fn check_noticed(
    pkg: &PkgScripts,
    script_name: &str,
    task: &Task,
    args: &[String],
    toolchain: &Toolchain,
) -> Result<(CacheCheck, Vec<String>), RunError> {
    let key = entry_key(script_name, task, args);
    let entry = cache_root(&pkg.dir).join(&key);
    let meta_path = entry.join("meta.json");
    // The non-git fallback warning applies even to early misses — the
    // probe is memoized and cheap.
    let mut notices = early_notices(pkg, task);
    if !meta_path.is_file() {
        return Ok((CacheCheck::Miss(MissReason::NoEntry), notices));
    }
    let meta: EntryMeta = match fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
    {
        Some(m) => m,
        None => return Ok((CacheCheck::Miss(MissReason::CorruptEntry), notices)),
    };
    if meta.version != CACHE_SCHEMA_VERSION {
        return Ok((CacheCheck::Miss(MissReason::SchemaChanged), notices));
    }

    // args → env → toolchain → inputs → output drift, in spec order.
    if meta.args != args {
        return Ok((CacheCheck::Miss(MissReason::ArgsChanged), notices));
    }
    let current_env = env_manifest(task);
    for (name, recorded) in &meta.env {
        match current_env.get(name) {
            Some(v) if v == recorded => {}
            _ => {
                return Ok((
                    CacheCheck::Miss(MissReason::EnvChanged(name.clone())),
                    notices,
                ));
            }
        }
    }
    // Names present now but absent from the manifest also count.
    for name in current_env.keys() {
        if !meta.env.contains_key(name) {
            return Ok((
                CacheCheck::Miss(MissReason::EnvChanged(name.clone())),
                notices,
            ));
        }
    }
    if meta.toolchain != *toolchain {
        return Ok((CacheCheck::Miss(MissReason::ToolchainChanged), notices));
    }

    // Inputs: union of current and recorded keys, sorted — first
    // culprit wins, deterministically.
    let scan = scan_inputs(pkg, task)?;
    notices.extend(scan.notices);
    let current = scan.files;
    let mut keys: Vec<&String> = current.keys().chain(meta.inputs.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        match (current.get(key), meta.inputs.get(key)) {
            (Some(_), None) => {
                return Ok((
                    CacheCheck::Miss(MissReason::InputAdded(key.clone())),
                    notices,
                ));
            }
            (None, Some(_)) => {
                return Ok((
                    CacheCheck::Miss(MissReason::InputRemoved(key.clone())),
                    notices,
                ));
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
                    return Ok((
                        CacheCheck::Miss(MissReason::InputModified(key.clone())),
                        notices,
                    ));
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
                    return Ok((CacheCheck::Miss(MissReason::OutputDrifted(rel)), notices));
                }
            }
        }
    }

    Ok((CacheCheck::Hit { entry, meta }, notices))
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
/// never a run failure. On success returns the input-scope notices
/// (the non-git fallback warning) so the caller can surface them —
/// store is the only scan on the first-run path.
pub fn store(
    pkg: &PkgScripts,
    script: &Script,
    task: &Task,
    args: &[String],
    toolchain: &Toolchain,
    outcome: &ExecOutcome,
) -> Result<Vec<String>, RunError> {
    debug_assert_eq!(outcome.code, 0, "failures are never cached");
    let key = entry_key(&script.name, task, args);
    let root = cache_root(&pkg.dir);
    fs::create_dir_all(&root)
        .map_err(|e| RunError::io(format!("cannot create {}: {e}", root.display())))?;

    // Fresh input manifest — every file hashed (store is rare; the
    // pre-filter only pays on lookup).
    let scan = scan_inputs(pkg, task)?;
    let mut inputs = BTreeMap::new();
    for (k, path) in &scan.files {
        inputs.insert(k.clone(), record_input(path)?);
    }
    let meta = EntryMeta {
        version: CACHE_SCHEMA_VERSION,
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
    Ok(scan.notices)
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
