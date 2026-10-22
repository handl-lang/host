//! Runtime configuration and appliers.
//!
//! This module provides the in-memory description of which kits and plugins a
//! host should load, what to exclude, and which option overrides to apply. It
//! also defines the `HostApplier` trait and a `.handl.toml` backed applier.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Number;

use crate::error::PlatformError;

/// A reference to a kit that should be loaded by the host.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KitReference {
    /// Kit identifier to include.
    pub include: String,
    /// Individual plugin ids to exclude from the included kit.
    pub exclude: Vec<String>,
    /// Optional path or registry location for the kit.
    pub path: Option<String>,
    /// Optional requested version of the kit.
    pub version: Option<String>,
}

/// A reference to a plugin that should be registered directly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginReference {
    /// Plugin identifier to register.
    pub register: String,
    /// Optional path to an external plugin manifest to load.
    pub manifest: Option<String>,
}

/// A project dependency declared in `[dependencies]` (or `[patch]`) of
/// `.handl.toml`.
///
/// The name is the import specifier's first path segment:
/// `import "arrows/transform/map.hnl"` resolves inside the `arrows`
/// dependency's directory.
///
/// A dependency has exactly one **source**:
///
/// - `path` — a vendored / sibling package directory containing `.hnl`
///   sources (relative to the manifest directory, or absolute).
/// - `git` — a repo-hosted package specifier (URL or
///   `host:owner/repo` shorthand, e.g. `github:handl-lang/c`) with an
///   optional `rev`/`branch`/`tag` pin. At resolve time the name maps
///   to the package's slot under [`package_cache_dir`], and the
///   fetch-on-miss hook (ADR-0010's registry half, [`fetch_dependencies`])
///   materializes the slot — `git clone` + `checkout` of the declared
///   selector — when it is absent. A populated slot is left untouched
///   (no refetch, no selector check), and a `[patch]` entry of the same
///   name overrides the source with a local checkout — the Cargo
///   `[patch]` dev-link story (`scripts/dev-link.sh`), so a *committed*
///   manifest can name the repo portably while a local machine resolves
///   the sibling dir.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencySpec {
    /// Path to the package root, relative to the manifest directory
    /// (or absolute). `None` when the dep is git-sourced.
    pub path: Option<String>,
    /// Git source specifier for a repo-hosted package. `None` when the
    /// dep is path-sourced.
    pub git: Option<String>,
    /// Exact revision pin for a git dep (`rev`/`branch`/`tag` are
    /// mutually exclusive in intent; at most one is meaningful).
    pub rev: Option<String>,
    /// Branch pin for a git dep.
    pub branch: Option<String>,
    /// Tag pin for a git dep.
    pub tag: Option<String>,
    /// Package root *inside* the source, when the package does not sit
    /// at the source's root — e.g. `subdir = "src"` for the `lib-*`
    /// repo layout, `subdir = "packages/catopt-kit"` for a monorepo
    /// package. Applies to both `path` and `git` sources (for `git`,
    /// the resolved dir is `<cache slot>/<subdir>`), and is what lets a
    /// `[patch]` entry pointing at a package-root checkout transparently
    /// override a git source: both resolve to the package root.
    pub subdir: Option<String>,
}

/// The in-memory description of which plugins to load, what to exclude, and
/// which option overrides to apply.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostConfig {
    /// Kits to include, each with optional per-kit exclusions.
    pub kits: Vec<KitReference>,
    /// Plugins to register directly on the host.
    pub plugins: Vec<PluginReference>,
    /// Plugin or kit ids to exclude from the final composition.
    pub excludes: Vec<String>,
    /// Option overrides keyed by dotted option id.
    pub options: HashMap<String, serde_json::Value>,
    /// Project dependencies keyed by import name (`[dependencies]` table):
    /// `import "<name>/<rest>"` resolves `<rest>` inside the named
    /// dependency's directory.
    pub dependencies: HashMap<String, DependencySpec>,
    /// Local-override table (`[patch]`): same `name → source` grammar as
    /// `[dependencies]`. A patch entry whose named directory exists wins
    /// over the `[dependencies]` source for that name — the consumer-side
    /// half of the git-package→dir bridge (dev-link to a sibling
    /// checkout). See [`resolve_dependencies`].
    pub patch: HashMap<String, DependencySpec>,
}

impl HostConfig {
    /// Creates a new, empty host configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a `.handl.toml` style configuration string.
    pub fn from_toml(source: &str) -> Result<Self, PlatformError> {
        let raw: RawHostConfig =
            toml::from_str(source).map_err(|e| PlatformError::InvalidMetadata(e.to_string()))?;

        Ok(Self {
            kits: raw
                .kit
                .into_iter()
                .map(|k| KitReference {
                    include: k.include,
                    exclude: k.exclude,
                    path: k.path,
                    version: k.version,
                })
                .collect(),
            plugins: raw
                .plugin
                .into_iter()
                .map(|p| PluginReference {
                    register: p.register,
                    manifest: p.manifest,
                })
                .collect(),
            excludes: raw.exclude,
            options: raw
                .options
                .into_iter()
                .map(|(k, v)| (k, toml_value_to_json(v)))
                .collect(),
            dependencies: parse_dependency_table("dependencies", raw.dependencies)?,
            patch: parse_dependency_table("patch", raw.patch)?,
        })
    }
}

/// Parse one `[dependencies]`/`[patch]` table into `name →
/// DependencySpec`, validating the single-source rule: an entry must
/// declare exactly one of `path`/`git`, and `rev`/`branch`/`tag` pins are
/// only meaningful on a `git` source.
fn parse_dependency_table(
    table: &str,
    raw: HashMap<String, RawDependency>,
) -> Result<HashMap<String, DependencySpec>, PlatformError> {
    raw.into_iter()
        .map(|(name, v)| {
            let spec = match v {
                RawDependency::Path(path) => RawDependencySpec {
                    path: Some(path),
                    ..RawDependencySpec::default()
                },
                RawDependency::Spec(s) => s,
            };
            if spec.path.is_some() && spec.git.is_some() {
                return Err(PlatformError::InvalidMetadata(format!(
                    "[{table}] entry `{name}` declares both `path` and `git` — \
                     a dependency has exactly one source"
                )));
            }
            if spec.path.is_none() && spec.git.is_none() {
                return Err(PlatformError::InvalidMetadata(format!(
                    "[{table}] entry `{name}` declares no source — \
                     a dependency needs `path` or `git`"
                )));
            }
            if spec.git.is_none()
                && (spec.rev.is_some() || spec.branch.is_some() || spec.tag.is_some())
            {
                return Err(PlatformError::InvalidMetadata(format!(
                    "[{table}] entry `{name}` pins `rev`/`branch`/`tag` without a \
                     `git` source — ref pins only apply to git dependencies"
                )));
            }
            Ok((
                name,
                DependencySpec {
                    path: spec.path,
                    git: spec.git,
                    rev: spec.rev,
                    branch: spec.branch,
                    tag: spec.tag,
                    subdir: spec.subdir,
                },
            ))
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
struct RawKit {
    include: String,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawPlugin {
    register: String,
    #[serde(default)]
    manifest: Option<String>,
}

/// A `[dependencies]`/`[patch]` entry: either a bare path string
/// (`arrows = "vendor/arrows"`) or a source table (`arrows = { path =
/// "vendor/arrows" }`, `host-tensor = { git = "…", tag = "v0.1" }`).
/// The table form is where source specifiers grow (hashes, tags) without
/// changing the schema.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawDependency {
    Path(String),
    Spec(RawDependencySpec),
}

/// The table form of a dependency entry. All fields are optional in the
/// grammar; [`parse_dependency_table`] validates the single-source rule.
#[derive(Debug, Clone, Default, Deserialize)]
struct RawDependencySpec {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    git: Option<String>,
    #[serde(default)]
    rev: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    subdir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawHostConfig {
    #[serde(default)]
    kit: Vec<RawKit>,
    #[serde(default)]
    plugin: Vec<RawPlugin>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    options: HashMap<String, toml::Value>,
    #[serde(default)]
    dependencies: HashMap<String, RawDependency>,
    #[serde(default)]
    patch: HashMap<String, RawDependency>,
}

/// Walk up from `start` (a file or directory) looking for a `.handl.toml`
/// project manifest; return the directory that contains it.
///
/// This is the discovery half of Phase 9.5 package management: the manifest
/// directory anchors both relative `[dependencies]` paths and the module
/// root for `import "path"` resolution.
pub fn discover_manifest_dir(start: &Path) -> Option<std::path::PathBuf> {
    let start_dir = if start.is_file() {
        start.parent().map(Path::to_path_buf)
    } else {
        Some(start.to_path_buf())
    };
    let start_dir = start_dir
        .and_then(|p| std::fs::canonicalize(p).ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    start_dir
        .ancestors()
        .find(|dir| dir.join(".handl.toml").is_file())
        .map(Path::to_path_buf)
}

/// The directory a `git`-sourced `[dependencies]` entry resolves to when
/// no `[patch]` override applies: `<package_cache_dir>/<name>` is the
/// slot [`fetch_dependencies`] materializes the package's `.hnl` tree
/// into (ADR-0010's registry half).
///
/// Location: `$HANDL_PKG_DIR` when set, otherwise `~/.handl/pkg`.
/// Keeping the unresolved name in the dep map — pointing at its cache
/// slot — means an `import` of a package whose fetch failed (or was
/// disabled) fails naming the directory the fetch populates, instead of
/// silently falling through to relative-path resolution.
pub fn package_cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("HANDL_PKG_DIR") {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".handl").join("pkg"))
        .unwrap_or_else(|| PathBuf::from(".handl").join("pkg"))
}

/// Resolve one manifest dependency entry to the directory it names.
///
/// A `path` source resolves relative to `manifest_dir` (canonicalized
/// when possible so module-cache keys are stable); a `git` source
/// resolves to its [`package_cache_dir`] slot — the location the package
/// fetch hook is expected to materialize.
fn dep_dir(manifest_dir: &Path, pkg_dir: &Path, name: &str, spec: &DependencySpec) -> PathBuf {
    match spec.path.as_deref() {
        Some(path) => {
            let p = PathBuf::from(path);
            let abs = if p.is_absolute() {
                p
            } else {
                manifest_dir.join(p)
            };
            let dir = match spec.subdir.as_deref() {
                Some(sub) => abs.join(sub),
                None => abs,
            };
            // Canonicalize when possible so the module cache keys are
            // stable; fall back to the joined path otherwise.
            std::fs::canonicalize(&dir).unwrap_or(dir)
        }
        // A git dep maps to its cache slot (plus `subdir` when the
        // package does not sit at the clone's root) — kept literal, not
        // canonicalized, so an `import` failure names the slot.
        None => match spec.subdir.as_deref() {
            Some(sub) => pkg_dir.join(name).join(sub),
            None => pkg_dir.join(name),
        },
    }
}

/// Read `<manifest_dir>/.handl.toml` and resolve its `[dependencies]` and
/// `[patch]` tables into an import-name → absolute package-directory map.
///
/// Dependency paths are relative to the manifest directory; absolute paths
/// are honored as-is. A missing manifest yields an empty map (no
/// dependencies). A present-but-unparseable manifest is an error — silent
/// fallback would make `import` failures look like missing files rather than
/// a broken manifest.
///
/// # The git-package→dir bridge
///
/// A `[dependencies]` entry may name its package **portably** by git
/// source — `host-tensor = { git = "https://github.com/handl-lang/lib-host-tensor",
/// tag = "v0.1" }` — instead of a machine-local `path`. Such an entry
/// resolves to its [`package_cache_dir`] slot, and resolution is
/// **fetch-on-miss**: when the slot is absent and no live `[patch]`
/// shadows the name, [`fetch_dependencies`] clones the declared repo and
/// checks out the selector before the map is built (ADR-0010's registry
/// half). The `[patch]` table is the consumer-supplied local override
/// for the same names — the Cargo `[patch]`/dev-link story
/// (`scripts/dev-link.sh`):
///
/// ```toml
/// [dependencies]
/// host-tensor = { git = "https://github.com/handl-lang/lib-host-tensor", tag = "v0.1" }
///
/// [patch]
/// host-tensor = "../lib-host-tensor"   # sibling checkout, machine-local
/// ```
///
/// A patch entry wins **when its named directory exists**; a patch that
/// names a missing directory is inert and the declared source applies,
/// so a committed manifest resolves the sibling checkout where one is
/// linked and the package cache elsewhere. `[patch]` may also name a
/// package `[dependencies]` does not declare — it simply enters the map
/// like a dep entry.
///
/// A failed fetch is **not** a resolution error: the name still maps to
/// its (absent) cache slot with a warning on stderr, so an `import` of
/// the package fails naming the directory the fetch would populate —
/// the same failure shape as before the fetch hook existed. A populated
/// slot is never refetched (no selector check — pins are trusted once
/// materialized), and `HANDL_PKG_FETCH=off` (or `0`/`false`/`no`)
/// disables the fetch entirely for offline/hermetic runs.
///
/// This is the seam that lets `.hnl` packages import each other *by name*
/// across repository boundaries (ADR-0128 Phase 5): `from "ws-c" import …`
/// or `from "libc/socket" import …` resolves through the table instead of a
/// `../` filesystem path, so a consuming repo declares the dep once in its
/// manifest — the Cargo-dep analogue for `.hnl` sources.
///
/// # Errors
///
/// Returns [`PlatformError::InvalidMetadata`] if the manifest exists but
/// cannot be read or parsed.
pub fn resolve_dependencies(
    manifest_dir: &Path,
) -> Result<HashMap<String, std::path::PathBuf>, PlatformError> {
    resolve_dependencies_in(manifest_dir, &package_cache_dir())
}

/// [`resolve_dependencies`] against an explicit package-cache directory —
/// the test seam that keeps `HANDL_PKG_DIR` env mutation out of the
/// suite.
fn resolve_dependencies_in(
    manifest_dir: &Path,
    pkg_dir: &Path,
) -> Result<HashMap<String, std::path::PathBuf>, PlatformError> {
    let manifest_path = manifest_dir.join(".handl.toml");
    let mut deps: HashMap<String, PathBuf> = if manifest_path.is_file() {
        let config = read_manifest(&manifest_path)?;
        let HostConfig {
            dependencies,
            patch,
            ..
        } = config;
        // The fetch hook runs before the map is built: a git-sourced dep
        // whose cache slot is absent (and which no live `[patch]`
        // shadows) is materialized in place, so the resolved directory
        // is usable on return.
        fetch_git_dependencies(manifest_dir, pkg_dir, &dependencies, &patch);
        let mut deps: HashMap<String, PathBuf> = dependencies
            .into_iter()
            .map(|(name, spec)| {
                let dir = dep_dir(manifest_dir, pkg_dir, &name, &spec);
                (name, dir)
            })
            .collect();
        // `[patch]` — the local-override table: a patch whose named
        // directory exists wins over the `[dependencies]` source for
        // that name; a missing patch dir is inert.
        for (name, spec) in patch {
            let dir = dep_dir(manifest_dir, pkg_dir, &name, &spec);
            if dir.is_dir() {
                deps.insert(name, dir);
            }
        }
        deps
    } else {
        HashMap::new()
    };
    // A package repo's own `pkg.hnl` fills any name the TOML manifest does not
    // declare (local development resolves it to the sibling `lib-<name>`).
    for (name, dir) in resolve_pkg_dependencies(manifest_dir)? {
        deps.entry(name).or_insert(dir);
    }
    Ok(deps)
}

/// Read + parse `<manifest>/.handl.toml`-style TOML into a [`HostConfig`].
fn read_manifest(manifest_path: &Path) -> Result<HostConfig, PlatformError> {
    let src = std::fs::read_to_string(manifest_path).map_err(|e| {
        PlatformError::InvalidMetadata(format!("cannot read {}: {e}", manifest_path.display()))
    })?;
    HostConfig::from_toml(&src).map_err(|e| {
        PlatformError::InvalidMetadata(format!("invalid {}: {e}", manifest_path.display()))
    })
}

/// A git-sourced dependency whose [`package_cache_dir`] slot the fetch
/// hook materialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedDependency {
    /// Import name the dependency is declared under.
    pub name: String,
    /// The cache slot the repo was cloned into.
    pub dir: PathBuf,
    /// The git URL cloned (after `github:`/`gitlab:`/`git+` shorthand
    /// expansion — see [`git_remote_url`]).
    pub url: String,
    /// The selector checked out, when the dep declared one
    /// (`rev`/`branch`/`tag`, in that precedence).
    pub selector: Option<String>,
}

/// Whether the fetch hook may run: `HANDL_PKG_FETCH` set to `0`, `off`,
/// `false`, or `no` disables it (offline or hermetic runs); unset or any
/// other value leaves it on.
fn package_fetch_enabled() -> bool {
    let Ok(v) = std::env::var("HANDL_PKG_FETCH") else {
        return true;
    };
    !matches!(
        v.to_ascii_lowercase().as_str(),
        "0" | "off" | "false" | "no"
    )
}

/// Expand a `git` source specifier into a URL `git clone` accepts.
///
/// `git+<url>` prefixes are stripped; the `github:owner/repo` and
/// `gitlab:owner/repo` shorthands expand to HTTPS URLs. Everything else
/// (`https://…`, `ssh://…`, `git@host:path`, `file://…`, bare local
/// paths) passes through to git untouched.
fn git_remote_url(spec: &str) -> String {
    let s = spec.strip_prefix("git+").unwrap_or(spec);
    match s.split_once(':') {
        Some(("github", rest)) => format!("https://github.com/{rest}"),
        Some(("gitlab", rest)) => format!("https://gitlab.com/{rest}"),
        _ => s.to_string(),
    }
}

/// The selector a git dep pins, in precedence order (`rev`/`branch`/`tag`
/// are mutually exclusive in intent).
fn git_selector(spec: &DependencySpec) -> Option<&str> {
    spec.rev
        .as_deref()
        .or(spec.branch.as_deref())
        .or(spec.tag.as_deref())
}

/// Run `git <args>` with `dir` as the working directory; `Err` carries
/// git's stderr (or the spawn failure) trimmed.
fn run_git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot spawn git: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Claim a unique private temp directory next to `slot` for an
/// out-of-place clone. `create_dir`'s exclusive semantics make the name
/// collision-proof across processes and threads; a dot prefix keeps
/// crash leftovers out of the slot's namespace (`live_git_deps` only
/// trusts the slot itself, so a stale temp never looks populated).
fn claim_temp_slot(parent: &Path, slot: &Path) -> Result<PathBuf, String> {
    let name = slot
        .file_name()
        .map_or_else(|| "dep".to_string(), |n| n.to_string_lossy().into_owned());
    for attempt in 0..64u32 {
        let tmp = parent.join(format!(
            ".{name}.fetch-{}-{attempt}",
            std::process::id(),
        ));
        match std::fs::create_dir(&tmp) {
            Ok(()) => return Ok(tmp),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!("cannot create temp slot {}: {e}", tmp.display()))
            }
        }
    }
    Err(format!(
        "cannot claim a temp slot next to {}",
        slot.display()
    ))
}

/// Clone `spec`'s git source, check out the declared selector, and
/// publish the result into `slot`.
///
/// The clone runs in a private temp directory ([`claim_temp_slot`]) and
/// is published with an atomic `rename`, so concurrent fetches of the
/// same dep (parallel `hnl` processes / test binaries resolving the same
/// `.handl.toml`) can never observe or destroy each other's clones: the
/// previous cleanup-on-failure removed the *shared* slot itself, so the
/// loser of a clone race deleted the winner's fresh checkout and the
/// winner's follow-up `git checkout` then failed with a bare
/// "No such file or directory" (the intermittent dep-checkout build
/// flake). A `rename` that finds `slot` already populated means another
/// fetch won the race — a populated slot is trusted by design
/// (fetch-on-miss never refetches), so that is success. Only the
/// caller's own temp dir is ever removed, on failure or on losing the
/// publish race; a crash mid-fetch leaves a dot-prefixed temp behind,
/// never a half-cloned slot that would be mistaken for populated.
fn materialize_git_dep(slot: &Path, spec: &DependencySpec) -> Result<(), String> {
    let url = git_remote_url(spec.git.as_deref().unwrap_or_default());
    let Some(parent) = slot.parent() else {
        return Err(format!("cache slot {} has no parent", slot.display()));
    };
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    let tmp = claim_temp_slot(parent, slot)?;
    let tmp_str = tmp.to_string_lossy().into_owned();
    let result = run_git(parent, &["clone", "--quiet", &url, &tmp_str])
        .and_then(|()| match git_selector(spec) {
            Some(sel) => run_git(&tmp, &["checkout", "--quiet", sel]),
            None => Ok(()),
        })
        .and_then(|()| match std::fs::rename(&tmp, slot) {
            Ok(()) => Ok(()),
            // Lost the publish race: the slot is populated by the other
            // fetch — same end state, so this is success. Drop our own
            // clone; the winner's slot stays untouched.
            Err(_e) if slot.is_dir() => {
                std::fs::remove_dir_all(&tmp).ok();
                Ok(())
            }
            Err(e) => Err(format!(
                "cannot publish git dependency clone {} -> {}: {e}",
                tmp.display(),
                slot.display()
            )),
        });
    if result.is_err() {
        std::fs::remove_dir_all(&tmp).ok();
    }
    result
}

/// The git-sourced deps that need materializing: declared `git` sources
/// whose [`package_cache_dir`] slot is absent and which no live `[patch]`
/// entry shadows (a patch whose directory exists keeps the dep local —
/// the fetch must not populate the slot behind the dev-link's back).
fn live_git_deps<'a>(
    manifest_dir: &Path,
    pkg_dir: &Path,
    dependencies: &'a HashMap<String, DependencySpec>,
    patch: &HashMap<String, DependencySpec>,
) -> Vec<(&'a str, &'a DependencySpec, PathBuf)> {
    dependencies
        .iter()
        .filter_map(|(name, spec)| {
            spec.git.as_ref()?;
            if let Some(patch_spec) = patch.get(name) {
                let dir = dep_dir(manifest_dir, pkg_dir, name, patch_spec);
                if dir.is_dir() {
                    return None;
                }
            }
            let slot = pkg_dir.join(name);
            if slot.is_dir() {
                return None;
            }
            Some((name.as_str(), spec, slot))
        })
        .collect()
}

/// Fetch every live git dep into its slot, warning on stderr (not
/// failing) when a clone does not succeed — the dep keeps its absent
/// slot so `import` errors still name it. Returns the deps materialized.
fn fetch_git_dependencies(
    manifest_dir: &Path,
    pkg_dir: &Path,
    dependencies: &HashMap<String, DependencySpec>,
    patch: &HashMap<String, DependencySpec>,
) -> Vec<FetchedDependency> {
    if !package_fetch_enabled() {
        return Vec::new();
    }
    live_git_deps(manifest_dir, pkg_dir, dependencies, patch)
        .into_iter()
        .filter_map(|(name, spec, slot)| {
            let url = git_remote_url(spec.git.as_deref().unwrap_or_default());
            match materialize_git_dep(&slot, spec) {
                Ok(()) => Some(FetchedDependency {
                    name: name.to_string(),
                    dir: slot,
                    url,
                    selector: git_selector(spec).map(str::to_string),
                }),
                Err(e) => {
                    eprintln!(
                        "warning: cannot fetch git dependency `{name}` from {url} \
                         into {}: {e}",
                        slot.display()
                    );
                    None
                }
            }
        })
        .collect()
}

/// Materialize a manifest's git-sourced `[dependencies]` into their
/// [`package_cache_dir`] slots — the standalone form of the fetch-on-miss
/// hook [`resolve_dependencies`] runs, for hosts that want to populate
/// the cache up front (a `hnl fetch`/`handl install` surface) rather
/// than on the resolution path.
///
/// A dep is skipped when its slot is already populated or a live
/// `[patch]` entry shadows it; a failed clone warns and is omitted from
/// the returned list. `HANDL_PKG_FETCH=off` disables fetching.
///
/// # Errors
///
/// Returns [`PlatformError::InvalidMetadata`] if the manifest exists but
/// cannot be read or parsed.
pub fn fetch_dependencies(manifest_dir: &Path) -> Result<Vec<FetchedDependency>, PlatformError> {
    fetch_dependencies_in(manifest_dir, &package_cache_dir())
}

/// [`fetch_dependencies`] against an explicit package-cache directory —
/// the test seam that keeps `HANDL_PKG_DIR` env mutation out of the
/// suite.
fn fetch_dependencies_in(
    manifest_dir: &Path,
    pkg_dir: &Path,
) -> Result<Vec<FetchedDependency>, PlatformError> {
    let manifest_path = manifest_dir.join(".handl.toml");
    if !manifest_path.is_file() {
        return Ok(Vec::new());
    }
    let config = read_manifest(&manifest_path)?;
    Ok(fetch_git_dependencies(
        manifest_dir,
        pkg_dir,
        &config.dependencies,
        &config.patch,
    ))
}

/// Discover the project manifest by walking up from `start` and resolve its
/// `[dependencies]`/`[patch]` tables. Equivalent to `discover_manifest_dir` +
/// [`resolve_dependencies`]; returns an empty map when no manifest exists.
///
/// # Errors
///
/// Returns [`PlatformError::InvalidMetadata`] if a discovered manifest
/// cannot be read or parsed.
pub fn discover_dependencies(
    start: &Path,
) -> Result<HashMap<String, std::path::PathBuf>, PlatformError> {
    match discover_manifest_dir(start) {
        Some(dir) => resolve_dependencies(&dir),
        None => Ok(HashMap::new()),
    }
}

/// Read `<manifest_dir>/pkg.hnl` and resolve its `deps` table.
///
/// `pkg.hnl` is the handl-native dependency manifest a package repo ships:
///
/// ```text
/// { deps: { math: "branch:main", "math-c": "branch:main" } }
/// ```
///
/// Each spec is `<kind>:<ref>`. Only the **local-development** half is
/// resolved here: `<name>` maps to the sibling checkout `../lib-<name>` (the
/// `lib-<package>` repo convention). Fetching `branch:`/`rev:`/`tag:` over git
/// is the package manager's job (ADR-0010), not the loader's, so the spec is
/// accepted but the ref is not acted on yet.
///
/// A missing file yields an empty map. A *malformed* `pkg.hnl` also yields an
/// empty map rather than an error: the file is authored by package repos this
/// workspace may not own, and a bad one would otherwise break unrelated
/// builds. `.handl.toml` stays the authoritative, validated manifest.
///
/// # Errors
///
/// Reserved for future validation; currently always `Ok`.
pub fn resolve_pkg_dependencies(
    manifest_dir: &Path,
) -> Result<HashMap<String, PathBuf>, PlatformError> {
    let path = manifest_dir.join("pkg.hnl");
    let Ok(src) = std::fs::read_to_string(&path) else {
        return Ok(HashMap::new());
    };
    let mut out = HashMap::new();
    for (name, _spec) in pkg_dep_specs(&src) {
        // Local-dev convention: the dep lives in the sibling `lib-<name>` repo.
        let dir = manifest_dir.join("..").join(format!("lib-{name}"));
        let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        out.insert(name, dir);
    }
    Ok(out)
}

/// `(name, spec)` pairs from a `pkg.hnl` source's `deps` table — the
/// source-level half of [`resolve_pkg_dependencies`], sharing its scan
/// (`pkg_deps_body`/`pkg_dep_entries`) without touching the filesystem.
///
/// Consumers that resolve the declared names somewhere other than the
/// sibling-`lib-<name>` convention (e.g. building a module environment
/// through a `ModuleResolver`) read the manifest without duplicating the
/// brace-balanced parse. A source with no `deps` table — or a malformed
/// one — yields an empty vec, the same leniency
/// [`resolve_pkg_dependencies`] applies so a bad manifest can never
/// break an unrelated build.
pub fn pkg_dep_specs(src: &str) -> Vec<(String, String)> {
    pkg_deps_body(src)
        .map(|body| pkg_dep_entries(&body))
        .unwrap_or_default()
}

/// The text between the braces of the `deps:` table in a `pkg.hnl` source.
///
/// Line comments (`--`) are stripped first; the scan is brace-balanced and
/// string-aware, so `"branch:main"` values cannot confuse it.
fn pkg_deps_body(src: &str) -> Option<String> {
    let src: String = src
        .lines()
        .map(|line| match line.find("--") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let deps_at = src.find("deps")?;
    let open = src[deps_at..].find('{')? + deps_at;
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    let mut start = None;
    for (i, c) in src[open..].char_indices() {
        let i = open + i;
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => {
                depth += 1;
                if depth == 1 {
                    start = Some(i + 1);
                }
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return start.map(|s| src[s..i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// `(name, spec)` pairs from the body of a `pkg.hnl` `deps` table.
///
/// Entries are comma-separated at depth zero; keys may be bare identifiers
/// (`math`) or quoted (`"math-c"`).
fn pkg_dep_entries(body: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    let mut current = String::new();
    let mut push = |entry: &str| {
        if let Some((k, v)) = entry.split_once(':') {
            let key = k.trim().trim_matches('"').trim();
            let val = v.trim().trim_matches('"').trim();
            if !key.is_empty() && !val.is_empty() {
                entries.push((key.to_string(), val.to_string()));
            }
        }
    };
    for c in body.chars() {
        if in_str {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                current.push(c);
            }
            '{' | '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            '}' | ')' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                push(&current);
                current.clear();
            }
            _ => current.push(c),
        }
    }
    push(&current);
    entries
}

fn toml_value_to_json(value: toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(i) => serde_json::Value::Number(Number::from(i)),
        toml::Value::Float(f) => Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
        toml::Value::Array(a) => {
            serde_json::Value::Array(a.into_iter().map(toml_value_to_json).collect())
        }
        toml::Value::Table(t) => serde_json::Value::Object(
            t.into_iter()
                .map(|(k, v)| (k, toml_value_to_json(v)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_config_parses_toml() {
        let source = r#"
[[kit]]
include = "handl-kit-canonical"
exclude = ["handl-plugin-deprecated-grammar"]

[[plugin]]
register = "handl-plugin-custom-linter"

[options]
"comment.style" = "above"
"no-unused-imports.level" = "error"
"experimental.lifetime-check" = false
"max-line-length" = 120
"threshold" = 3.14
"features" = ["a", "b"]
"nested" = { key = "value" }
"#;

        let config = HostConfig::from_toml(source).unwrap();

        assert_eq!(config.kits.len(), 1);
        assert_eq!(config.kits[0].include, "handl-kit-canonical");
        assert_eq!(
            config.kits[0].exclude,
            vec!["handl-plugin-deprecated-grammar"]
        );

        assert_eq!(config.plugins.len(), 1);
        assert_eq!(config.plugins[0].register, "handl-plugin-custom-linter");

        assert_eq!(config.options.get("comment.style").unwrap(), "above");
        assert_eq!(
            config.options.get("no-unused-imports.level").unwrap(),
            "error"
        );
        assert_eq!(
            config.options.get("experimental.lifetime-check").unwrap(),
            &serde_json::Value::Bool(false)
        );
        assert_eq!(
            config.options.get("max-line-length").unwrap(),
            &serde_json::Value::Number(Number::from(120))
        );
        assert!(config.options.get("threshold").unwrap().is_number());
        assert!(config.options.get("features").unwrap().is_array());
        assert!(config.options.get("nested").unwrap().is_object());
    }

    #[test]
    fn host_config_parses_dependencies() {
        let source = r#"
[dependencies]
arrows = "vendor/arrows"
utils = { path = "../shared/utils" }
"#;

        let config = HostConfig::from_toml(source).unwrap();

        assert_eq!(config.dependencies.len(), 2);
        assert_eq!(
            config.dependencies["arrows"].path.as_deref(),
            Some("vendor/arrows")
        );
        assert_eq!(
            config.dependencies["utils"].path.as_deref(),
            Some("../shared/utils")
        );
    }

    /// The git-package→dir bridge grammar: a `[dependencies]` entry may
    /// name its package by git source, and `[patch]` carries the local
    /// override (`name → dir`) — the Cargo `[patch]` dev-link story.
    #[test]
    fn host_config_parses_git_deps_and_patch() {
        let source = r#"
[dependencies]
host-tensor = { git = "https://github.com/handl-lang/lib-host-tensor", tag = "v0.1" }
arrows = "vendor/arrows"

[patch]
host-tensor = "../lib-host-tensor"
vendored = { path = "../lib-vendored" }
"#;

        let config = HostConfig::from_toml(source).unwrap();

        let git_dep = &config.dependencies["host-tensor"];
        assert_eq!(
            git_dep.git.as_deref(),
            Some("https://github.com/handl-lang/lib-host-tensor")
        );
        assert_eq!(git_dep.tag.as_deref(), Some("v0.1"));
        assert_eq!(git_dep.path, None);
        assert_eq!(
            config.dependencies["arrows"].path.as_deref(),
            Some("vendor/arrows")
        );

        assert_eq!(
            config.patch["host-tensor"].path.as_deref(),
            Some("../lib-host-tensor")
        );
        assert_eq!(
            config.patch["vendored"].path.as_deref(),
            Some("../lib-vendored")
        );
    }

    /// A dependency entry must declare exactly one source (`path` xor
    /// `git`), and ref pins require a git source.
    #[test]
    fn dependency_source_is_validated() {
        assert!(
            HostConfig::from_toml("[dependencies]\nx = { path = \"a\", git = \"b\" }\n").is_err(),
            "path + git is ambiguous"
        );
        assert!(
            HostConfig::from_toml("[dependencies]\nx = {}\n").is_err(),
            "no source at all"
        );
        assert!(
            HostConfig::from_toml("[dependencies]\nx = { path = \"a\", tag = \"v1\" }\n").is_err(),
            "a ref pin without a git source"
        );
        // The same rules apply to `[patch]` entries.
        assert!(HostConfig::from_toml("[patch]\nx = {}\n").is_err());
    }

    /// `[patch]` is the local override: a patch entry whose directory
    /// exists wins over the declared `[dependencies]` source — whether
    /// that source is a git spec (→ the package-cache slot) or a vendored
    /// path. A patch naming a missing directory is inert.
    #[test]
    fn resolve_dependencies_patch_overrides_declared_source() {
        let root = std::env::temp_dir().join(format!("hnl-patch-{}", std::process::id()));
        let proj = root.join("repo");
        let sibling = root.join("lib-host-tensor");
        let vendored = proj.join("vendor/arrows");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::create_dir_all(&vendored).unwrap();
        std::fs::write(
            proj.join(".handl.toml"),
            concat!(
                "[dependencies]\n",
                "host-tensor = { git = \"https://github.com/handl-lang/lib-host-tensor\", tag = \"v0.1\" }\n",
                "arrows = \"vendor/arrows\"\n",
                // A git URL that resolves nowhere, so the fetch hook's
                // clone attempt fails fast and hermetically.
                "unpatched-git = { git = \"file:///nonexistent-handl-dep/lib-upstream\" }\n",
                "\n[patch]\n",
                // Sibling checkout exists → overrides the git source.
                "host-tensor = \"../lib-host-tensor\"\n",
                // Missing dir → inert; `arrows` keeps its vendored path.
                "arrows = \"../lib-arrows\"\n",
                // Patch may name a package `[dependencies]` does not declare.
                "extra = \"../lib-host-tensor\"\n",
            ),
        )
        .unwrap();

        let deps = resolve_dependencies(&proj).unwrap();
        assert_eq!(
            deps["host-tensor"],
            std::fs::canonicalize(&sibling).unwrap()
        );
        assert_eq!(deps["arrows"], std::fs::canonicalize(&vendored).unwrap());
        assert_eq!(deps["extra"], std::fs::canonicalize(&sibling).unwrap());
        // An unpatched git dep resolves to its package-cache slot — the
        // dir the fetch hook (follow-up seam) materializes.
        assert_eq!(
            deps["unpatched-git"],
            package_cache_dir().join("unpatched-git")
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A `git`-sourced dep keeps a stable cache-slot key in the dep map
    /// even when the fetch hook cannot populate it — an `import` of the
    /// name fails naming the slot a fetch would populate. (The URL
    /// resolves nowhere so the clone attempt fails fast and offline.)
    #[test]
    fn unpatched_git_dependency_maps_to_the_package_cache() {
        let root = std::env::temp_dir().join(format!("hnl-gitdep-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(".handl.toml"),
            "[dependencies]\nws-c = { git = \"file:///nonexistent-handl-dep/ws-c\", rev = \"abc123\" }\n",
        )
        .unwrap();

        let deps = resolve_dependencies(&root).unwrap();
        assert_eq!(deps["ws-c"], package_cache_dir().join("ws-c"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn discover_dependencies_resolves_relative_to_manifest_dir() {
        let root = std::env::temp_dir().join(format!("hnl-deps-{}", std::process::id()));
        let dep_dir = root.join("vendor/ws-c");
        let nested = root.join("src/nested");
        std::fs::create_dir_all(&dep_dir).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            root.join(".handl.toml"),
            "[dependencies]\nws-c = \"vendor/ws-c\"\n",
        )
        .unwrap();

        // Discovery walks up from a nested directory to the manifest.
        let deps = discover_dependencies(&nested).unwrap();
        assert_eq!(deps["ws-c"], std::fs::canonicalize(&dep_dir).unwrap());
        assert_eq!(
            discover_manifest_dir(&nested).unwrap(),
            std::fs::canonicalize(&root).unwrap()
        );

        // No manifest → empty map.
        let bare = std::env::temp_dir().join(format!("hnl-deps-bare-{}", std::process::id()));
        std::fs::create_dir_all(&bare).unwrap();
        assert!(discover_dependencies(&bare).unwrap().is_empty());

        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&bare).ok();
    }

    /// `pkg.hnl`'s `deps` table resolves each name to the sibling
    /// `lib-<name>` checkout (the local-development half of the seam).
    #[test]
    fn resolve_pkg_dependencies_maps_sibling_repos() {
        let root = std::env::temp_dir().join(format!("hnl-pkgdeps-{}", std::process::id()));
        let proj = root.join("repo");
        let math_dep = root.join("lib-math");
        let math_c_dep = root.join("lib-math-c");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&math_dep).unwrap();
        std::fs::create_dir_all(&math_c_dep).unwrap();
        std::fs::write(
            proj.join("pkg.hnl"),
            "--- deps of this package\n{ deps: { math: \"branch:main\", \"math-c\": \"branch:main\" } }\n",
        )
        .unwrap();

        let deps = resolve_pkg_dependencies(&proj).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps["math"], std::fs::canonicalize(&math_dep).unwrap());
        assert_eq!(deps["math-c"], std::fs::canonicalize(&math_c_dep).unwrap());

        std::fs::remove_dir_all(&root).ok();
    }

    /// A missing or malformed `pkg.hnl` is not an error — it must never break
    /// an unrelated build.
    #[test]
    fn resolve_pkg_dependencies_is_lenient() {
        let root = std::env::temp_dir().join(format!("hnl-pkgdeps-bad-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(resolve_pkg_dependencies(&root).unwrap().is_empty());

        std::fs::write(root.join("pkg.hnl"), "{ deps: { math: ").unwrap();
        assert!(resolve_pkg_dependencies(&root).unwrap().is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    /// `pkg_dep_specs` reads the `deps` table straight from a `pkg.hnl`
    /// source — the source-level half [`resolve_pkg_dependencies`] is
    /// built on.
    #[test]
    fn pkg_dep_specs_reads_deps_from_source() {
        let src = "--- deps of this package\n{ deps: { math: \"branch:main\", \"math-c\": \"branch:main\" } }\n";
        assert_eq!(
            pkg_dep_specs(src),
            vec![
                ("math".to_string(), "branch:main".to_string()),
                ("math-c".to_string(), "branch:main".to_string()),
            ]
        );
        // No `deps` table — or a malformed one — is leniently empty.
        assert!(pkg_dep_specs("{ modules: [\"cli\"] }").is_empty());
        assert!(pkg_dep_specs("").is_empty());
        assert!(pkg_dep_specs("{ deps: { math: ").is_empty());
    }

    /// `.handl.toml` wins; `pkg.hnl` only fills names it does not declare.
    #[test]
    fn resolve_dependencies_merges_pkg_hnl_without_overriding_toml() {
        let root = std::env::temp_dir().join(format!("hnl-merge-{}", std::process::id()));
        let proj = root.join("repo");
        let vendored = proj.join("vendor/math");
        let sibling = root.join("lib-rand");
        std::fs::create_dir_all(&vendored).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(
            proj.join(".handl.toml"),
            "[dependencies]\nmath = \"vendor/math\"\n",
        )
        .unwrap();
        std::fs::write(
            proj.join("pkg.hnl"),
            "{ deps: { math: \"branch:main\", rand: \"branch:main\" } }\n",
        )
        .unwrap();

        let deps = resolve_dependencies(&proj).unwrap();
        assert_eq!(deps["math"], std::fs::canonicalize(&vendored).unwrap());
        assert_eq!(deps["rand"], std::fs::canonicalize(&sibling).unwrap());

        std::fs::remove_dir_all(&root).ok();
    }

    // ------------------------------------------------------------------
    // The fetch hook (ADR-0010 registry half)
    // ------------------------------------------------------------------

    /// Serializes tests that exercise the fetch hook: the
    /// `HANDL_PKG_FETCH=off` case mutates a process-global env var the
    /// positive cases depend on being unset.
    static FETCH_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A scratch git repo with one committed `index.hnl` on `main`;
    /// returns `(repo dir, HEAD sha)`.
    fn fixture_git_repo(root: &Path, name: &str) -> (PathBuf, String) {
        let repo = root.join(name);
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .output()
                .expect("git runs");
            assert!(out.status.success(), "git {args:?}: {:?}", out.stderr);
        };
        git(&["init", "--quiet", "-b", "main"]);
        std::fs::write(repo.join("index.hnl"), "export x := 1\n").unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test",
            "commit",
            "--quiet",
            "-m",
            "init",
        ]);
        let sha = String::from_utf8(
            std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&repo)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        (repo, sha)
    }

    fn head_sha(dir: &Path) -> String {
        String::from_utf8(
            std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(dir)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string()
    }

    /// Fetch-on-miss, end to end: a git dep whose slot is absent is
    /// cloned into `<pkg_dir>/<name>` and the declared selector checked
    /// out — one arm per selector kind (`branch`, `tag`, `rev`, none).
    #[test]
    fn resolve_fetches_a_git_dep_into_its_cache_slot() {
        let _guard = FETCH_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("hnl-fetch-{}", std::process::id()));
        let proj = root.join("repo");
        let pkg = root.join("pkg");
        std::fs::create_dir_all(&proj).unwrap();
        let (repo, sha) = fixture_git_repo(&root, "upstream");
        let url = format!("file://{}", repo.display());
        std::process::Command::new("git")
            .args(["tag", "v0.1"])
            .current_dir(&repo)
            .output()
            .unwrap();

        for (name, sel) in [
            ("by-branch", Some(("branch", "main"))),
            ("by-tag", Some(("tag", "v0.1"))),
            ("by-rev", Some(("rev", sha.as_str()))),
            ("no-pin", None),
        ] {
            let entry = match sel {
                Some((kind, v)) => format!("{name} = {{ git = \"{url}\", {kind} = \"{v}\" }}"),
                None => format!("{name} = {{ git = \"{url}\" }}"),
            };
            std::fs::write(
                proj.join(".handl.toml"),
                format!("[dependencies]\n{entry}\n"),
            )
            .unwrap();

            let deps = resolve_dependencies_in(&proj, &pkg).unwrap();
            let slot = pkg.join(name);
            assert_eq!(deps[name], slot);
            assert!(
                slot.join("index.hnl").is_file(),
                "the slot must hold the cloned package"
            );
            assert_eq!(head_sha(&slot), sha, "{name} checked out the selector");
            std::fs::remove_dir_all(&slot).unwrap();
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `subdir` names the package root inside the source: a git dep on a
    /// repo whose package lives at `src/` resolves `<slot>/src`, so a
    /// `[patch]` to a package-root checkout overrides it transparently.
    #[test]
    fn resolve_fetches_a_git_dep_with_subdir() {
        let _guard = FETCH_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("hnl-subdir-{}", std::process::id()));
        let proj = root.join("repo");
        let pkg = root.join("pkg");
        let sibling = root.join("sibling");
        std::fs::create_dir_all(&proj).unwrap();
        // Repo layout: the package root is `src/`, not the clone root —
        // the `lib-*` convention.
        let (repo, _) = fixture_git_repo(&root, "upstream-src");
        let srcdir = repo.join("src");
        std::fs::create_dir_all(&srcdir).unwrap();
        std::fs::write(srcdir.join("index.hnl"), "export y := 2\n").unwrap();
        for args in [
            &["add", "."][..],
            &[
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@test",
                "commit",
                "--quiet",
                "-m",
                "pkg under src/",
            ][..],
        ] {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?} failed");
        }
        let url = format!("file://{}", repo.display());
        std::fs::create_dir_all(&sibling).unwrap();

        std::fs::write(
            proj.join(".handl.toml"),
            format!("[dependencies]\ndep = {{ git = \"{url}\", subdir = \"src\" }}\n"),
        )
        .unwrap();

        let deps = resolve_dependencies_in(&proj, &pkg).unwrap();
        assert_eq!(deps["dep"], pkg.join("dep").join("src"));
        assert!(pkg.join("dep/src/index.hnl").is_file());

        // A live patch to a package-root checkout resolves to the same
        // shape the git source does — the override is transparent.
        std::fs::write(
            proj.join(".handl.toml"),
            format!(
                "[dependencies]\ndep = {{ git = \"{url}\", subdir = \"src\" }}\n\
                 \n[patch]\ndep = \"../sibling\"\n"
            ),
        )
        .unwrap();
        let deps = resolve_dependencies_in(&proj, &pkg).unwrap();
        assert_eq!(deps["dep"], std::fs::canonicalize(&sibling).unwrap());

        // `subdir` composes with a path source too.
        let vendored = proj.join("vendor/x/src");
        std::fs::create_dir_all(&vendored).unwrap();
        std::fs::write(
            proj.join(".handl.toml"),
            "[dependencies]\nv = { path = \"vendor/x\", subdir = \"src\" }\n",
        )
        .unwrap();
        let deps = resolve_dependencies_in(&proj, &pkg).unwrap();
        assert_eq!(deps["v"], std::fs::canonicalize(&vendored).unwrap());

        std::fs::remove_dir_all(&root).ok();
    }

    /// A populated slot is never refetched — and a live `[patch]` keeps
    /// the dep local without touching the cache at all.
    #[test]
    fn populated_slots_and_live_patches_skip_the_fetch() {
        let _guard = FETCH_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("hnl-skip-{}", std::process::id()));
        let proj = root.join("repo");
        let pkg = root.join("pkg");
        let patch_dir = root.join("sibling");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&patch_dir).unwrap();
        let (repo, _) = fixture_git_repo(&root, "upstream");
        let url = format!("file://{}", repo.display());

        // A hand-populated slot: contents left alone, no refetch even
        // though it is not a git checkout of the declared source.
        let slot = pkg.join("pinned");
        std::fs::create_dir_all(&slot).unwrap();
        std::fs::write(slot.join("marker"), "kept").unwrap();

        std::fs::write(
            proj.join(".handl.toml"),
            format!(
                "[dependencies]\npinned = {{ git = \"{url}\" }}\n\
                 patched = {{ git = \"{url}\" }}\n\
                 \n[patch]\npatched = \"../sibling\"\n"
            ),
        )
        .unwrap();

        let deps = resolve_dependencies_in(&proj, &pkg).unwrap();
        assert_eq!(deps["pinned"], slot);
        assert!(slot.join("marker").is_file(), "populated slot untouched");
        assert!(
            !slot.join("index.hnl").exists(),
            "no clone landed over the populated slot"
        );
        assert_eq!(
            deps["patched"],
            std::fs::canonicalize(&patch_dir).unwrap(),
            "a live patch wins; the slot is not materialized"
        );
        assert!(
            !pkg.join("patched").exists(),
            "a patch-shadowed git dep must not fetch"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A fetch that cannot complete is soft: resolution still maps the
    /// name to its (absent) slot and no half-clone is left behind.
    #[test]
    fn a_failed_fetch_keeps_the_slot_absent() {
        let _guard = FETCH_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("hnl-nofetch-{}", std::process::id()));
        let proj = root.join("repo");
        let pkg = root.join("pkg");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(
            proj.join(".handl.toml"),
            "[dependencies]\ngone = { git = \"file:///nonexistent-handl-dep/gone\" }\n",
        )
        .unwrap();

        let deps = resolve_dependencies_in(&proj, &pkg).unwrap();
        assert_eq!(deps["gone"], pkg.join("gone"));
        assert!(!pkg.join("gone").exists(), "failed clone leaves no slot");

        std::fs::remove_dir_all(&root).ok();
    }

    /// Concurrent fetches of the same git dep must not destroy each
    /// other's clones (BACKLOG: dep-checkout build flakes). The
    /// fetch-on-miss hook used to clean up a failed clone by
    /// `remove_dir_all`ing the *shared* slot; when several resolvers
    /// raced on the same absent slot (parallel `hnl` processes / test
    /// binaries resolving one `.handl.toml`), the loser's cleanup
    /// deleted the winner's fresh clone and the winner's `git checkout`
    /// then failed with a bare "No such file or directory". The clone
    /// now runs in a private temp slot published by an atomic rename, so
    /// every racer either publishes or defers to the winner — and no
    /// dot-prefixed temp may linger afterwards.
    #[test]
    fn concurrent_fetches_keep_the_slot_intact() {
        let _guard = FETCH_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("hnl-race-{}", std::process::id()));
        let proj = root.join("repo");
        let pkg = root.join("pkg");
        std::fs::create_dir_all(&proj).unwrap();
        let (repo, sha) = fixture_git_repo(&root, "upstream");
        let url = format!("file://{}", repo.display());
        std::fs::write(
            proj.join(".handl.toml"),
            format!("[dependencies]\ndep = {{ git = \"{url}\", branch = \"main\" }}\n"),
        )
        .unwrap();

        const RACERS: usize = 8;
        const ROUNDS: usize = 4;
        for _ in 0..ROUNDS {
            std::fs::remove_dir_all(&pkg).ok();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(RACERS));
            let handles: Vec<_> = (0..RACERS)
                .map(|_| {
                    let barrier = std::sync::Arc::clone(&barrier);
                    let proj = proj.clone();
                    let pkg = pkg.clone();
                    std::thread::spawn(move || {
                        barrier.wait();
                        // `PlatformError` is not `Send` (it carries an
                        // `Rc`-backed `EvalContext`), so reduce to a
                        // plain count / error string at the boundary.
                        fetch_dependencies_in(&proj, &pkg)
                            .map(|fetched| fetched.len())
                            .map_err(|e| e.to_string())
                    })
                })
                .collect();
            let mut reported = 0usize;
            for handle in handles {
                let fetched = handle
                    .join()
                    .expect("racer thread")
                    .expect("fetch survives the race");
                if fetched == 1 {
                    reported += 1;
                }
            }

            let slot = pkg.join("dep");
            assert!(
                slot.join("index.hnl").is_file(),
                "the winner's clone must survive the race"
            );
            assert_eq!(head_sha(&slot), sha, "the selector stays checked out");
            assert!(
                reported >= 1,
                "at least one racer must report the materialized dep"
            );

            // No dot-prefixed temp leftovers once every racer finished.
            for entry in std::fs::read_dir(&pkg).unwrap() {
                let name = entry.unwrap().file_name();
                assert!(
                    !name.to_string_lossy().starts_with('.'),
                    "no temp slot may linger: {name:?}"
                );
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `HANDL_PKG_FETCH=off` turns the hook off entirely — the dep maps
    /// to its slot with no clone attempted (offline/hermetic runs).
    #[test]
    fn handl_pkg_fetch_off_disables_the_hook() {
        let _guard = FETCH_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("hnl-off-{}", std::process::id()));
        let proj = root.join("repo");
        let pkg = root.join("pkg");
        std::fs::create_dir_all(&proj).unwrap();
        let (repo, _) = fixture_git_repo(&root, "upstream");
        std::fs::write(
            proj.join(".handl.toml"),
            format!(
                "[dependencies]\ndep = {{ git = \"file://{}\" }}\n",
                repo.display()
            ),
        )
        .unwrap();

        // SAFETY: serialized by FETCH_ENV_LOCK; restored before unlock.
        unsafe { std::env::set_var("HANDL_PKG_FETCH", "off") };
        let deps = resolve_dependencies_in(&proj, &pkg).unwrap();
        unsafe { std::env::remove_var("HANDL_PKG_FETCH") };

        assert_eq!(deps["dep"], pkg.join("dep"));
        assert!(!pkg.join("dep").exists(), "fetch disabled → no clone");

        std::fs::remove_dir_all(&root).ok();
    }

    /// The standalone materialization surface: `fetch_dependencies`
    /// reports what it cloned, skips populated slots, and warns through
    /// clone failures without erroring the manifest.
    #[test]
    fn fetch_dependencies_reports_materialized_slots() {
        let _guard = FETCH_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("hnl-pubfetch-{}", std::process::id()));
        let proj = root.join("repo");
        let pkg = root.join("pkg");
        std::fs::create_dir_all(&proj).unwrap();
        let (repo, sha) = fixture_git_repo(&root, "upstream");
        let url = format!("file://{}", repo.display());
        std::fs::write(
            proj.join(".handl.toml"),
            format!(
                "[dependencies]\nok = {{ git = \"{url}\", rev = \"{sha}\" }}\n\
                 bad = {{ git = \"file:///nonexistent-handl-dep/bad\" }}\n"
            ),
        )
        .unwrap();

        let fetched = fetch_dependencies_in(&proj, &pkg).unwrap();
        assert_eq!(fetched.len(), 1, "only the cloneable dep reports");
        let dep = &fetched[0];
        assert_eq!(dep.name, "ok");
        assert_eq!(dep.dir, pkg.join("ok"));
        assert_eq!(dep.url, url);
        assert_eq!(dep.selector.as_deref(), Some(sha.as_str()));

        // Idempotent: the populated slot is skipped on the second call.
        assert!(fetch_dependencies_in(&proj, &pkg).unwrap().is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    /// `github:`/`gitlab:` shorthands and `git+` prefixes expand; real
    /// URLs and scp-style specs pass through untouched.
    #[test]
    fn git_remote_url_expands_shorthands() {
        assert_eq!(
            git_remote_url("github:handl-lang/c"),
            "https://github.com/handl-lang/c"
        );
        assert_eq!(
            git_remote_url("gitlab:acme/kit"),
            "https://gitlab.com/acme/kit"
        );
        assert_eq!(
            git_remote_url("git+https://git.example.com/x"),
            "https://git.example.com/x"
        );
        assert_eq!(
            git_remote_url("https://github.com/handl-lang/c"),
            "https://github.com/handl-lang/c"
        );
        assert_eq!(
            git_remote_url("git@github.com:handl-lang/c"),
            "git@github.com:handl-lang/c"
        );
        assert_eq!(git_remote_url("file:///tmp/r"), "file:///tmp/r");
    }

    /// `rev` beats `branch` beats `tag` when several pins are present.
    #[test]
    fn git_selector_precedence() {
        let spec = DependencySpec {
            rev: Some("r".into()),
            branch: Some("b".into()),
            tag: Some("t".into()),
            ..DependencySpec::default()
        };
        assert_eq!(git_selector(&spec), Some("r"));
        let spec = DependencySpec {
            branch: Some("b".into()),
            tag: Some("t".into()),
            ..DependencySpec::default()
        };
        assert_eq!(git_selector(&spec), Some("b"));
        let spec = DependencySpec {
            tag: Some("t".into()),
            ..DependencySpec::default()
        };
        assert_eq!(git_selector(&spec), Some("t"));
        assert_eq!(git_selector(&DependencySpec::default()), None);
    }
}
