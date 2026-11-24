//! Runtime configuration and appliers.
//!
//! This module provides the in-memory description of which kits and plugins a
//! host should load, what to exclude, and which option overrides to apply. It
//! also defines the `HostApplier` trait and a `.handl.toml` backed applier.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
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
            kits: parse_kit_field(raw.kit)?,
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

/// The `kit` key of `.handl.toml` accepts two shapes (T-122):
///
/// - `[[kit]]` tables — host-kit composition entries, which become
///   [`HostConfig::kits`];
/// - `kit = "<name-or-path>"` — the eval-kit selection key consumed by
///   [`discover_kit_spec`] (`hnl run`/`hnl eval`). It names a `kit.hnl`
///   kit for the eval-time module environment — a different namespace
///   from host-kit composition — so it contributes no entries here.
///
/// Kept as a raw [`toml::Value`] so both spellings parse; anything else
/// is a manifest error.
fn parse_kit_field(raw: Option<toml::Value>) -> Result<Vec<KitReference>, PlatformError> {
    let Some(value) = raw else {
        return Ok(Vec::new());
    };
    match value {
        toml::Value::String(_) => Ok(Vec::new()),
        toml::Value::Array(entries) => entries
            .into_iter()
            .map(|entry| {
                let k: RawKit = entry.try_into().map_err(|e| {
                    PlatformError::InvalidMetadata(format!("invalid [[kit]] entry: {e}"))
                })?;
                Ok(KitReference {
                    include: k.include,
                    exclude: k.exclude,
                    path: k.path,
                    version: k.version,
                })
            })
            .collect(),
        other => Err(PlatformError::InvalidMetadata(format!(
            "`kit` must be `kit = \"<name-or-path>\"` or [[kit]] tables, got {other}"
        ))),
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
    kit: Option<toml::Value>,
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

/// Walk up from `start` (a file or directory) looking for a `pkg.hnl`
/// package manifest; return the directory that contains it.
///
/// The `pkg.hnl` counterpart of [`discover_manifest_dir`]: `pkg.hnl` is
/// the handl-native package manifest (`modules`/`deps`/`scripts`), so
/// package-local surfaces — the `hnl run <script>` task runner — resolve
/// it by the same walk-up convention. Like [`resolve_pkg_dependencies`],
/// a malformed `pkg.hnl` is the caller's lenient-scan problem, not this
/// function's: discovery only answers *where* the nearest manifest is.
pub fn discover_pkg_dir(start: &Path) -> Option<std::path::PathBuf> {
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
        .find(|dir| dir.join("pkg.hnl").is_file())
        .map(Path::to_path_buf)
}

/// A `.handl.toml` `kit = "<spec>"` selection discovered on disk (T-122).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredKit {
    /// Directory containing the `.handl.toml` that declared `kit` —
    /// relative specs resolve against it (the same convention
    /// `[dependencies]` paths follow).
    pub manifest_dir: PathBuf,
    /// The `kit =` value verbatim: a kit directory path or a bare
    /// `kit-<name>` name, resolved by the caller exactly like `--kit`.
    pub spec: String,
    /// The `extends =` link of the kit-declaring manifest, resolved to
    /// the `.handl.toml` it names — `None` when that manifest declares
    /// no `extends` (or the link does not resolve). Recorded for
    /// forward-compat: non-kit keys will layer along this link.
    pub extends: Option<PathBuf>,
}

/// The user-level config directory consulted by [`discover_kit_spec`]'s
/// global fallback: `$HANDL_CONFIG_DIR` when set (the test seam, matching
/// [`package_cache_dir`]'s `HANDL_PKG_DIR`), otherwise
/// `~/.config/handl` (`%USERPROFILE%\.config\handl` off-Unix).
pub fn handl_config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("HANDL_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".config").join("handl"))
}

/// The user-level `.handl.toml` — `~/.config/handl/.handl.toml` — the
/// global fallback for [`discover_kit_spec`]. `None` when no config home
/// resolves or the file is absent.
pub fn global_manifest_path() -> Option<PathBuf> {
    let path = handl_config_dir()?.join(".handl.toml");
    path.is_file().then_some(path)
}

/// The `kit`/`extends` string keys of a `.handl.toml`, parsed leniently.
///
/// Reads the top-level keys only when they are strings — `[[kit]]`
/// host-kit tables, non-string values, and manifests without the keys
/// all yield `None` for that key. An unparseable manifest yields `None`
/// for both: the file's own [`resolve_dependencies`] path reports the
/// parse error with its path, so discovery stays lenient rather than
/// shadowing that diagnostic.
#[derive(Debug, Default)]
struct ManifestKeys {
    /// The `kit = "<name-or-path>"` eval-kit selection (T-122).
    kit: Option<String>,
    /// The `extends = "<path-or-spec>"` inheritance link.
    extends: Option<String>,
}

fn manifest_keys(src: &str) -> Option<ManifestKeys> {
    let value: toml::Value = toml::from_str(src).ok()?;
    let string_key = |key: &str| {
        value
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    Some(ManifestKeys {
        kit: string_key("kit"),
        extends: string_key("extends"),
    })
}

/// Maximum `extends` hops followed while discovering a kit — a bound on
/// pathological chains. Cycles are reported separately (a manifest
/// revisited through the chain is always an error, never a silent stop).
const MAX_EXTENDS_DEPTH: usize = 8;

/// Resolve a `.handl.toml` `extends = "<path-or-spec>"` value to the
/// manifest file it names.
///
/// Path-like values (absolute, or containing a `/`/`\` separator)
/// resolve relative to `manifest_dir`; a directory target means its
/// `.handl.toml`, a file target is used as-is. A bare name follows the
/// kit-spec convention — probe `<manifest_dir>/kit-<name>` then
/// `<manifest_dir>/../kit-<name>` — and names the `.handl.toml` inside
/// (mirroring `resolve_kit_dir`'s sibling-checkout convention).
///
/// `Err` when nothing resolves: a dangling `extends` is a manifest bug,
/// not a reason to silently skip the link.
fn resolve_extends_manifest(manifest_dir: &Path, value: &str) -> Result<PathBuf, PlatformError> {
    let path = Path::new(value);
    if !path.is_absolute() && !value.contains('/') && !value.contains('\\') {
        let parent = manifest_dir.parent().unwrap_or(manifest_dir);
        for base in [manifest_dir, parent] {
            let candidate = base.join(format!("kit-{value}")).join(".handl.toml");
            if candidate.is_file() {
                return Ok(std::fs::canonicalize(&candidate).unwrap_or(candidate));
            }
        }
        return Err(PlatformError::InvalidMetadata(format!(
            "extends `{value}`: no kit-{value}/.handl.toml next to {}",
            manifest_dir.display()
        )));
    }
    let base = if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_dir.join(path)
    };
    let candidate = if base.is_dir() {
        base.join(".handl.toml")
    } else {
        base
    };
    if candidate.is_file() {
        Ok(std::fs::canonicalize(&candidate).unwrap_or(candidate))
    } else {
        Err(PlatformError::InvalidMetadata(format!(
            "extends `{value}`: {} does not exist",
            candidate.display()
        )))
    }
}

/// Follow a manifest's `extends` chain looking for the first `kit =`
/// key. `entry` is the `.handl.toml` path of a manifest already known
/// to lack `kit` — the chain is inspected starting *at* it (a defensive
/// re-read covers callers that hand over an uninspected manifest).
///
/// Returns the [`DiscoveredKit`] of the first chained manifest
/// declaring `kit` — its own `manifest_dir` anchors the spec, and its
/// own `extends` is recorded on the result — or `None` when the chain
/// ends without a `kit` (the caller then continues its normal ancestor
/// walk; `extends` supplies fallback keys, it does not redirect the
/// walk).
///
/// A manifest revisited through the chain is a cycle and an `extends`
/// chain longer than [`MAX_EXTENDS_DEPTH`] hops is too deep — both are
/// [`PlatformError::InvalidMetadata`] errors naming the manifests.
fn kit_via_extends_chain(entry: &Path) -> Result<Option<DiscoveredKit>, PlatformError> {
    let mut visited = HashSet::new();
    let mut current = entry.to_path_buf();
    let mut hops = 0usize;
    loop {
        let canonical = std::fs::canonicalize(&current).unwrap_or_else(|_| current.clone());
        if !visited.insert(canonical) {
            return Err(PlatformError::InvalidMetadata(format!(
                "extends cycle: {} reaches {} again",
                entry.display(),
                current.display()
            )));
        }
        let manifest_dir = current.parent().map(Path::to_path_buf).unwrap_or_default();
        let manifest_dir = std::fs::canonicalize(&manifest_dir).unwrap_or(manifest_dir);
        let Ok(src) = std::fs::read_to_string(&current) else {
            return Ok(None);
        };
        let Some(keys) = manifest_keys(&src) else {
            return Ok(None);
        };
        if let Some(spec) = keys.kit {
            let extends = keys
                .extends
                .as_deref()
                .and_then(|e| resolve_extends_manifest(&manifest_dir, e).ok());
            return Ok(Some(DiscoveredKit {
                manifest_dir,
                spec,
                extends,
            }));
        }
        let Some(link) = keys.extends else {
            return Ok(None);
        };
        hops += 1;
        if hops > MAX_EXTENDS_DEPTH {
            return Err(PlatformError::InvalidMetadata(format!(
                "extends chain from {} exceeds depth {MAX_EXTENDS_DEPTH}",
                entry.display()
            )));
        }
        current = resolve_extends_manifest(&manifest_dir, &link)?;
    }
}

/// Inspect one `.handl.toml`: a `kit =` key on the manifest itself wins
/// immediately (its `extends`, when present, is only recorded); failing
/// that, an `extends` link is followed via [`kit_via_extends_chain`].
/// `Ok(None)` when the manifest is unreadable, unparseable, or yields
/// no kit through its chain.
fn kit_declared_or_extended(manifest: &Path) -> Result<Option<DiscoveredKit>, PlatformError> {
    let Ok(src) = std::fs::read_to_string(manifest) else {
        return Ok(None);
    };
    let Some(keys) = manifest_keys(&src) else {
        return Ok(None);
    };
    let manifest_dir = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
    if let Some(spec) = keys.kit {
        let extends = keys
            .extends
            .as_deref()
            .and_then(|e| resolve_extends_manifest(&manifest_dir, e).ok());
        let manifest_dir = std::fs::canonicalize(&manifest_dir).unwrap_or(manifest_dir);
        return Ok(Some(DiscoveredKit {
            manifest_dir,
            spec,
            extends,
        }));
    }
    if keys.extends.is_some() {
        return kit_via_extends_chain(manifest);
    }
    Ok(None)
}

/// T-122 kit discovery: walk up from `start` (a file or directory —
/// `hnl run` passes the target `.hnl` file, `hnl eval` its cwd) looking
/// for the nearest `.handl.toml` whose top level declares
/// `kit = "<name-or-path>"`. Manifests without the key do not stop the
/// walk. When no ancestor qualifies, fall back to the user-level
/// [`global_manifest_path`] (`~/.config/handl/.handl.toml`), which uses
/// the same key.
///
/// `extends = "<path-or-spec>"` inheritance (T-122 follow-up): a
/// manifest without `kit` but with `extends` hands discovery to the
/// manifest it names — resolved relative to the manifest's directory,
/// bare names probing `kit-<name>` like kit specs — and the first
/// chained manifest declaring `kit` wins (its `manifest_dir` anchors
/// the spec). A manifest declaring both keeps its own `kit`; its
/// `extends` is recorded on [`DiscoveredKit::extends`] for future
/// non-kit layering. Extends chains are cycle-checked and capped at
/// [`MAX_EXTENDS_DEPTH`] hops; [`discover_kit_spec_checked`] surfaces
/// those as errors while this lenient wrapper returns `None`.
///
/// The returned [`DiscoveredKit::spec`] is *not* resolved to a
/// directory here — the caller resolves it against
/// [`DiscoveredKit::manifest_dir`] exactly like an explicit `--kit`
/// value (path-or-name), so `--kit` remains a strict override and a
/// discovered spec that names no kit fails the same way.
pub fn discover_kit_spec(start: &Path) -> Option<DiscoveredKit> {
    discover_kit_spec_checked(start).ok().flatten()
}

/// [`discover_kit_spec`] with `extends`-chain diagnostics: `Err` on an
/// `extends` cycle, an over-deep chain, or an unresolvable `extends`
/// target; `Ok(None)` when nothing yields a kit.
pub fn discover_kit_spec_checked(start: &Path) -> Result<Option<DiscoveredKit>, PlatformError> {
    discover_kit_spec_in_checked(start, global_manifest_path().as_deref())
}

/// [`discover_kit_spec`] against an explicit global-manifest path — the
/// test seam that keeps `$HOME`/`HANDL_CONFIG_DIR` env mutation out of
/// the suite. The checked variant is [`discover_kit_spec_in_checked`].
fn discover_kit_spec_in(start: &Path, global: Option<&Path>) -> Option<DiscoveredKit> {
    discover_kit_spec_in_checked(start, global).ok().flatten()
}

fn discover_kit_spec_in_checked(
    start: &Path,
    global: Option<&Path>,
) -> Result<Option<DiscoveredKit>, PlatformError> {
    let start_dir = if start.is_file() {
        start.parent().map(Path::to_path_buf)
    } else {
        Some(start.to_path_buf())
    };
    let start_dir = start_dir
        .and_then(|p| std::fs::canonicalize(p).ok())
        .unwrap_or_else(|| PathBuf::from("."));

    for dir in start_dir.ancestors() {
        let manifest = dir.join(".handl.toml");
        if !manifest.is_file() {
            continue;
        }
        if let Some(found) = kit_declared_or_extended(&manifest)? {
            return Ok(Some(found));
        }
    }

    let Some(global) = global else {
        return Ok(None);
    };
    kit_declared_or_extended(global)
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
        let tmp = parent.join(format!(".{name}.fetch-{}-{attempt}", std::process::id(),));
        match std::fs::create_dir(&tmp) {
            Ok(()) => return Ok(tmp),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("cannot create temp slot {}: {e}", tmp.display())),
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
        out.insert(name.clone(), sibling_lib_dir(manifest_dir, &name));
    }
    Ok(out)
}

/// First `key:` occurrence in comment-stripped `pkg.hnl` source.
///
/// `key` must stand alone (no identifier char before it) and be followed
/// by optional whitespace then `:`, and quoted strings are skipped — so
/// annotation prose like `description: "deps: see docs"` or a bare word
/// in `keywords: [deps]` cannot shadow the real table.
fn pkg_key_at(src: &str, key: &str) -> Option<usize> {
    let bytes = src.as_bytes();
    let key = key.as_bytes();
    let mut i = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    while i + key.len() <= bytes.len() {
        let b = bytes[i];
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            in_str = true;
            i += 1;
            continue;
        }
        let boundary = i == 0
            || !matches!(
                bytes[i - 1],
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'
            );
        if boundary && bytes[i..].starts_with(key) {
            let mut j = i + key.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Strip `--` line comments from `pkg.hnl` source, string-aware: a `--`
/// inside a `"…"` literal is content, not a comment — script commands
/// like `"hnl codegen plan --check"` depend on it. The pre-string-aware
/// per-line `find("--")` truncation used by the older scanners would
/// cut those commands in half.
fn pkg_strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_str = false;
    let mut escaped = false;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
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
                out.push(c);
            }
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                for c2 in chars.by_ref() {
                    if c2 == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// The text between the braces of a top-level `key: { … }` table in a
/// `pkg.hnl` source.
///
/// Line comments (`--`) are stripped first; the scan is brace-balanced and
/// string-aware, so `"branch:main"` values cannot confuse it.
fn pkg_brace_body(src: &str, key: &str) -> Option<String> {
    let src = pkg_strip_comments(src);
    let deps_at = pkg_key_at(&src, key)?;
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

/// The text between the braces of the `deps:` table in a `pkg.hnl` source.
fn pkg_deps_body(src: &str) -> Option<String> {
    pkg_brace_body(src, "deps")
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

// ---------------------------------------------------------------------------
// pkg.hnl atom/module-scoped deps (P-013 T-150)
//
// A `deps` entry's value may be the legacy `<kind>:<ref>` spec string —
// the whole dependency surface — or a record narrowing the surface the
// consuming package sees (tsconfig `lib`/`types` semantics):
//
// ```text
// deps: {
//   mri:   "branch:main",                              -- whole dep (legacy)
//   proc:  { spec: "branch:main", atoms: ["proc.argv"] },
//   utils: { modules: ["math", "str"] },               -- whole modules only
//   both:  { spec: "rev:abc", modules: ["math"], atoms: ["math.abs"] },
// }
// ```
//
// - `spec` (or `ref`) — the fetch spec, same grammar as the string form;
//   absent means the local-development default (`branch:main`).
// - `modules` — dep module names exposed whole. Module names are the
//   second `/`-segment of an import specifier (`from "utils/math"` →
//   module `math`); a bare dep name (`from "mri"`) scopes the dep's
//   primary module, named after the dep.
// - `atoms` — `module.field` exports exposed individually. When `atoms`
//   is non-empty the allowed modules are implied by the atom qualifiers;
//   combined with `modules`, a module must pass both gates and only the
//   listed fields of it are exposed.
//
// A bare-string value naming a *module* (`deps: { "lib-utils": "math" }`,
// the coarser form sketched in the proposal) is deliberately NOT adopted:
// a colon-less string is already meaningful fetch-spec grammar
// (`dep_spec_ref` defaults it to `branch:main`), so overloading it would
// silently change resolution of existing manifests. `modules:`/`atoms:`
// records are the unambiguous spelling.
// ---------------------------------------------------------------------------

/// The declared surface a `pkg.hnl` `deps` entry restricts a dependency
/// to (P-013 T-150). Both lists empty = the whole dep, today's behavior.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PkgDepScope {
    /// Dep module names exposed whole (`modules: ["math"]`).
    pub modules: Vec<String>,
    /// `module.field` exports exposed individually (`atoms: ["math.abs"]`).
    pub atoms: Vec<String>,
}

impl PkgDepScope {
    /// Whether the scope restricts anything — `false` means the dep's
    /// whole surface is exposed (the legacy behavior).
    pub fn is_unrestricted(&self) -> bool {
        self.modules.is_empty() && self.atoms.is_empty()
    }

    /// Whether `module` (a dep module name — see [`dep_spec_scope`]) is
    /// inside the scope at all: it must pass the `modules` gate and, when
    /// `atoms` are declared, be named by at least one atom qualifier.
    pub fn allows_module(&self, module: &str) -> bool {
        if !self.modules.is_empty() && !self.modules.iter().any(|m| m == module) {
            return false;
        }
        self.atoms.is_empty()
            || self
                .atoms
                .iter()
                .any(|a| a.split('.').next() == Some(module))
    }

    /// Whether `module.field` is an exposed export under this scope.
    pub fn allows_field(&self, module: &str, field: &str) -> bool {
        if !self.allows_module(module) {
            return false;
        }
        self.atoms.is_empty()
            || self
                .atoms
                .iter()
                .any(|a| a == &format!("{module}.{field}"))
    }
}

/// One `pkg.hnl` `deps` entry with its optional surface scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkgDep {
    /// The dep name — the import specifier's first `/`-segment.
    pub name: String,
    /// The fetch spec (`"branch:main"`, `"rev:…"`, …); empty when the
    /// record form omits `spec`/`ref` (local-development default).
    pub spec: String,
    /// The declared surface restriction; unrestricted for the legacy
    /// string form.
    pub scope: PkgDepScope,
}

/// `(name, spec)` pairs from a `pkg.hnl` source's `deps` table — see
/// [`pkg_dep_scopes`] for the scope-carrying form. Kept for callers that
/// only need the fetch spec; a record value contributes its `spec`/`ref`
/// field (empty when absent).
pub fn pkg_dep_specs(src: &str) -> Vec<(String, String)> {
    pkg_dep_scopes(src)
        .into_iter()
        .map(|d| (d.name, d.spec))
        .collect()
}

/// Every `deps` entry of a `pkg.hnl` source with its scope — the
/// scope-carrying half of [`pkg_dep_specs`] (P-013 T-150).
///
/// Same leniency: no `deps` table or a malformed one yields an empty vec.
pub fn pkg_dep_scopes(src: &str) -> Vec<PkgDep> {
    let Some(body) = pkg_deps_body(src) else {
        return Vec::new();
    };
    pkg_dep_entries(&body)
        .into_iter()
        .map(|(name, value)| {
            let value = value.trim();
            if value.starts_with('{') {
                let (spec, scope) = pkg_dep_scope_record(value);
                PkgDep { name, spec, scope }
            } else {
                PkgDep {
                    name,
                    spec: value.to_string(),
                    scope: PkgDepScope::default(),
                }
            }
        })
        .collect()
}

/// `name → scope` for the *restricted* deps of a `pkg.hnl` source — the
/// map consumers key import specifiers against. Unrestricted entries are
/// omitted: absence means "whole surface", so lookups stay a single
/// `HashMap::get`.
pub fn pkg_dep_scope_map(src: &str) -> HashMap<String, PkgDepScope> {
    pkg_dep_scopes(src)
        .into_iter()
        .filter(|d| !d.scope.is_unrestricted())
        .map(|d| (d.name, d.scope))
        .collect()
}

/// The dep scopes declared by the `pkg.hnl` next to the `.handl.toml`
/// [`discover_manifest_dir`] finds from `start` — the same manifest pair
/// [`resolve_dependencies`] merges. Empty when no manifest or no
/// `pkg.hnl` exists (or none of its deps are scoped).
///
/// [`discover_manifest_dir`]: crate::config::discover_manifest_dir
/// [`resolve_dependencies`]: crate::config::resolve_dependencies
pub fn discover_dep_scopes(start: &Path) -> HashMap<String, PkgDepScope> {
    let Some(dir) = discover_manifest_dir(start) else {
        return HashMap::new();
    };
    let Ok(src) = std::fs::read_to_string(dir.join("pkg.hnl")) else {
        return HashMap::new();
    };
    pkg_dep_scope_map(&src)
}

/// Parse a record-form dep value (`{ spec: "…", modules: […], atoms: […] }`)
/// into `(spec, scope)`. Unknown keys are ignored; a missing `spec`/`ref`
/// yields an empty spec (the `dep_spec_ref` local-development default).
fn pkg_dep_scope_record(value: &str) -> (String, PkgDepScope) {
    let inner = value
        .trim()
        .strip_prefix('{')
        .and_then(|v| v.strip_suffix('}'))
        .unwrap_or(value);
    let mut spec = String::new();
    let mut scope = PkgDepScope::default();
    for (key, val) in pkg_dep_entries(inner) {
        match key.as_str() {
            "spec" | "ref" => spec = val,
            "modules" => scope.modules = pkg_dep_name_list(&val),
            "atoms" => scope.atoms = pkg_dep_name_list(&val),
            _ => {}
        }
    }
    (spec, scope)
}

/// Quoted-name entries of a `[ "a", "b" ]` list value — brackets stripped,
/// then the same lenient split [`hnl_list_entries`] applies.
fn pkg_dep_name_list(value: &str) -> Vec<String> {
    let v = value.trim();
    let inner = v
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(v);
    hnl_list_entries(inner)
}

/// The `(scope, dep name, module name)` an import specifier resolves to
/// under `scopes`, or `None` when the spec's first segment names no
/// *restricted* dep.
///
/// The first `/`-separated segment is the dep name (a `pkg.hnl` `deps`
/// key); the second — when present — is the module inside a multi-module
/// package (`"utils/math"` → dep `utils`, module `math`). A bare dep name
/// (`"mri"`, `"mri/index"`) scopes the dep's primary module, named after
/// the dep itself.
pub fn dep_spec_scope<'a>(
    scopes: &'a HashMap<String, PkgDepScope>,
    spec: &str,
) -> Option<(&'a PkgDepScope, String, String)> {
    let (first, rest) = match spec.split_once('/') {
        Some((f, r)) => (f, Some(r)),
        None => (spec, None),
    };
    let scope = scopes.get(first)?;
    let module = rest
        .and_then(|r| r.split('/').next())
        .map(|m| m.trim_end_matches(".hnl").to_string())
        .filter(|m| !m.is_empty() && m != "index")
        .unwrap_or_else(|| first.to_string());
    Some((scope, first.to_string(), module))
}

/// Filter a module bindings record (`TaggedValue::Obj`) to the fields
/// `scope` exposes for `module` — the shared half of dep scoping
/// (P-013 T-150) used by both the `ModuleLoader` eval path and
/// scope-wrapping `Module`s. Non-object bindings pass through unchanged
/// (there is nothing to filter); an object keeps only the entries
/// [`PkgDepScope::allows_field`] admits.
pub fn scope_tagged_bindings(
    bindings: &std::rc::Rc<handl_value::TaggedValue>,
    scope: &PkgDepScope,
    module: &str,
) -> std::rc::Rc<handl_value::TaggedValue> {
    use handl_value::TaggedValue;
    let TaggedValue::Obj(map) = bindings.as_ref() else {
        return std::rc::Rc::clone(bindings);
    };
    if scope.is_unrestricted() {
        return std::rc::Rc::clone(bindings);
    }
    // Clone + retain keeps the concrete `FxHashMap` type unnameable here.
    let mut filtered = (**map).clone();
    filtered.retain(|k, _| scope.allows_field(module, k.as_ref()));
    std::rc::Rc::new(TaggedValue::Obj(std::rc::Rc::new(filtered)))
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

/// The `modules` list of a `pkg.hnl` source, in declaration order.
///
/// Empty when the table is absent or malformed — the same leniency
/// [`pkg_dep_specs`] applies (a bad manifest must not break an unrelated build).
pub fn pkg_modules(src: &str) -> Vec<String> {
    let Some(body) = pkg_list_body(src, "modules") else {
        return Vec::new();
    };
    hnl_list_entries(&body)
}

/// The `entry` override of a `pkg.hnl` source, when declared.
pub fn pkg_entry(src: &str) -> Option<String> {
    let clean: String = src
        .lines()
        .map(|l| match l.find("--") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let at = pkg_key_at(&clean, "entry")?;
    let tail = &clean[at + "entry".len()..];
    let colon = tail.find(':')?;
    let value = tail[colon + 1..].trim_start();
    if !value.starts_with('"') {
        return None;
    }
    let end = value[1..].find('"')? + 1;
    Some(value[1..end].to_string())
}

/// Where a module lives inside a package (ADR-0160).
///
/// One module resolves to `src/index.hnl`; N modules resolve to
/// `src/<name>/index.hnl` each; `entry` overrides the one-module case.
pub fn pkg_module_path(modules: &[String], entry: Option<&str>, module: &str) -> Option<String> {
    if modules.len() == 1 && modules[0] == module {
        // Legacy packages predate the `src/index.hnl` rule and name the file
        // after the module (`src/type-handl.hnl`); the caller resolves the
        // fallback by trying the candidates in order.
        return Some(entry.unwrap_or("src/index.hnl").to_string());
    }
    if modules.iter().any(|m| m == module) {
        return Some(format!("src/{module}/index.hnl"));
    }
    None
}

/// Candidate files for a module, most canonical first.
///
/// `src/index.hnl` is the rule (ADR-0160); `src/<module>.hnl` is the legacy
/// 1-module shape most of the 113 repos still use, and
/// `src/<module>/index.hnl` the multi-module one.
pub fn pkg_module_candidates(modules: &[String], entry: Option<&str>, module: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(path) = pkg_module_path(modules, entry, module) {
        out.push(path);
    }
    if modules.iter().any(|m| m == module) {
        for candidate in [
            format!("src/{module}.hnl"),
            format!("src/{module}/index.hnl"),
        ] {
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }
    out
}

/// Split a `pkg.hnl` dep spec (`branch:main`, `rev:abc`, `tag:v1`) into
/// `(kind, value)`. Unknown or empty specs fall back to `("branch", "main")`.
pub fn dep_spec_ref(spec: &str) -> (&'static str, String) {
    match spec.split_once(':') {
        Some((kind, value)) if !value.trim().is_empty() => match kind.trim() {
            "rev" => ("rev", value.trim().to_string()),
            "tag" => ("tag", value.trim().to_string()),
            _ => ("branch", {
                let v = value.trim();
                if v.is_empty() {
                    "main".to_string()
                } else {
                    v.to_string()
                }
            }),
        },
        _ => ("branch", "main".to_string()),
    }
}

/// Repo candidates for a dep `name`, most specific first: the package repo,
/// then the pure-Handl impl, then the backends (`-c`, `-native`).
pub fn dep_repo_candidates(name: &str) -> Vec<String> {
    vec![
        format!("lib-{name}"),
        format!("lib-{name}-handl"),
        format!("lib-{name}-c"),
        format!("lib-{name}-native"),
    ]
}

/// The text between the brackets of a top-level `key: [ … ]` list.
fn pkg_list_body(src: &str, key: &str) -> Option<String> {
    let clean: String = src
        .lines()
        .map(|l| match l.find("--") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    let at = pkg_key_at(&clean, key)?;
    let open = clean[at..].find('[')? + at;
    let close = clean[open..].find(']')? + open;
    Some(clean[open + 1..close].to_string())
}

// ---------------------------------------------------------------------------
// pkg.hnl `scripts:` — the `hnl run <name>` task-runner manifest (P-003)
//
// A script value has three forms:
//
// - `"cmd line"` — one command line (whitespace-split argv, `&&` separates
//   sequential commands);
// - `["cmd1", "cmd2"]` — sequential command lines;
// - `{ run: …, dependsOn: […], cache: bool, inputs/outputs/env: […], cwd }`
//   — a cacheable task.
//
// Like the rest of the `pkg_*` family the scan is lenient: comments are
// stripped, strings are escape-aware, and a missing or malformed table
// yields an empty vec rather than an error.
// ---------------------------------------------------------------------------

/// A `scripts:` entry of `pkg.hnl`, parsed into its value form.
#[derive(Debug, Clone, PartialEq)]
pub enum PkgScriptValue {
    /// `"cmd line"` — a single command line.
    Command(String),
    /// `["cmd1", "cmd2"]` — sequential command lines.
    Commands(Vec<String>),
    /// `{ run: …, … }` — a task that may declare `dependsOn`/`cache`/
    /// `inputs`/`outputs`/`env`/`cwd`.
    Task(PkgScriptTask),
}

/// The object form of a `pkg.hnl` script — a runnable task with optional
/// caching and dependency declarations.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PkgScriptTask {
    /// `run` — one command line (string) or several (array), in order.
    pub run: Vec<String>,
    /// `dependsOn` — names of sibling scripts that must run first.
    pub depends_on: Vec<String>,
    /// `cache` — `None` when undeclared (the runner defaults tasks to
    /// cached; scripts/arrays never cache).
    pub cache: Option<bool>,
    /// `inputs` — glob list fingerprinted into the cache key; `None`
    /// when undeclared (the runner substitutes its own default).
    pub inputs: Option<Vec<String>>,
    /// `outputs` — glob list of artifacts restored from / snapshotted
    /// into the cache entry; `None`/empty disables file restoration.
    pub outputs: Option<Vec<String>>,
    /// `env` — environment variable names additionally fingerprinted.
    pub env: Vec<String>,
    /// `cwd` — working directory relative to the package root.
    pub cwd: Option<String>,
}

/// One named entry of a `pkg.hnl` `scripts` table.
#[derive(Debug, Clone, PartialEq)]
pub struct PkgScript {
    /// The script name — a bare identifier or quoted string.
    pub name: String,
    /// The parsed value form.
    pub value: PkgScriptValue,
}

/// The `scripts` table of a `pkg.hnl` source, in declaration order.
///
/// Empty when the table is absent or malformed — the same leniency
/// [`pkg_dep_specs`] applies (a bad manifest must not break an unrelated
/// build). Entries whose value parses to none of the three forms are
/// dropped.
pub fn pkg_scripts(src: &str) -> Vec<PkgScript> {
    pkg_brace_body(src, "scripts")
        .map(|body| pkg_script_entries(&body))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(name, value)| {
            parse_pkg_script_value(&value).map(|v| PkgScript { name, value: v })
        })
        .collect()
}

/// `(name, raw-value)` pairs from the body of a `pkg.hnl` `scripts`
/// table.
///
/// Entries are comma-separated at depth zero; unlike [`pkg_dep_entries`]
/// the value is kept verbatim (it may be a string, an array, or a nested
/// record) for [`parse_pkg_script_value`] to interpret.
fn pkg_script_entries(body: &str) -> Vec<(String, String)> {
    split_hnl_entries(body)
        .into_iter()
        .filter_map(|entry| hnl_field_split(&entry))
        .map(|(k, v)| {
            let key = k.trim().trim_matches('"').trim().to_string();
            (key, v)
        })
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

/// Split a record/array body into its top-level comma-separated entries,
/// honoring strings and nested `()`/`[]`/`{}` — the depth-aware half of
/// [`pkg_dep_entries`] generalized to keep raw text.
fn split_hnl_entries(body: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    let mut current = String::new();
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
                entries.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    entries.push(current);
    entries
        .into_iter()
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
        .collect()
}

/// Split one `key: value` entry at the first `:` outside a string —
/// returns `(key, value)` both trimmed, or `None` when no separator.
fn hnl_field_split(entry: &str) -> Option<(String, String)> {
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in entry.char_indices() {
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
            ':' => {
                return Some((
                    entry[..i].trim().to_string(),
                    entry[i + 1..].trim().to_string(),
                ));
            }
            _ => {}
        }
    }
    None
}

/// The inner text of a `[…]`/`{…}` literal occupying `text`'s first
/// non-whitespace char — string-aware and nesting-aware. `None` on an
/// unterminated literal.
fn hnl_inner(text: &str) -> Option<&str> {
    let t = text.trim_start();
    let (open, close) = match t.as_bytes().first()? {
        b'[' => ('[', ']'),
        b'{' => ('{', '}'),
        _ => return None,
    };
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in t.char_indices() {
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
            _ if c == open => depth += 1,
            _ if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(&t[1..i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse a `"…"` string literal, unescaping `\"`, `\\`, `\n`, `\t`,
/// `\r`; other escapes keep the escaped char. `None` when `text` is not
/// a well-formed quoted literal.
fn hnl_string_literal(text: &str) -> Option<String> {
    let t = text.trim();
    let inner = t.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    let mut closed = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                closed = true;
                break;
            }
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => return None,
            },
            _ => out.push(c),
        }
    }
    if !closed || !chars.as_str().trim().is_empty() {
        return None;
    }
    Some(out)
}

/// String elements of a `[…]` list body (the text *inside* the brackets),
/// in order. Non-string elements are skipped — the same leniency the
/// `pkg_*` scanners apply everywhere.
fn hnl_string_list(body: &str) -> Vec<String> {
    split_hnl_entries(body)
        .into_iter()
        .filter_map(|e| hnl_string_literal(&e))
        .collect()
}

/// A string-or-list-of-strings value (`"x"` → `vec!["x"]`, `["a","b"]`
/// → both) for the task fields `dependsOn`/`inputs`/`outputs`/`env`.
fn hnl_string_or_list(text: &str) -> Vec<String> {
    let t = text.trim();
    if let Some(inner) = hnl_inner(t) {
        if t.trim_start().starts_with('[') {
            return hnl_string_list(inner);
        }
        // A `{…}` is not a string list — ignore it.
        return Vec::new();
    }
    hnl_string_literal(t).into_iter().collect()
}

/// Command lines of a task's `run` field — a string becomes one line,
/// an array its elements in order.
fn hnl_command_lines(text: &str) -> Vec<String> {
    let t = text.trim();
    if t.trim_start().starts_with('[') {
        return hnl_inner(t).map(hnl_string_list).unwrap_or_default();
    }
    hnl_string_literal(t).into_iter().collect()
}

/// Parse one `scripts:` value into its [`PkgScriptValue`] form.
///
/// `None` when the value is empty or a task record without a usable
/// `run` — the entry is then dropped by [`pkg_scripts`].
fn parse_pkg_script_value(text: &str) -> Option<PkgScriptValue> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with('"') {
        return hnl_string_literal(t).map(PkgScriptValue::Command);
    }
    if t.starts_with('[') {
        let inner = hnl_inner(t)?;
        return Some(PkgScriptValue::Commands(hnl_string_list(inner)));
    }
    if t.starts_with('{') {
        let inner = hnl_inner(t)?;
        let mut task = PkgScriptTask::default();
        for entry in split_hnl_entries(inner) {
            let Some((k, v)) = hnl_field_split(&entry) else {
                continue;
            };
            match k.trim().trim_matches('"').trim() {
                "run" => task.run = hnl_command_lines(&v),
                "dependsOn" => task.depends_on = hnl_string_or_list(&v),
                "inputs" => task.inputs = Some(hnl_string_or_list(&v)),
                "outputs" => task.outputs = Some(hnl_string_or_list(&v)),
                "env" => task.env = hnl_string_or_list(&v),
                "cache" => {
                    task.cache = match v.trim() {
                        "true" => Some(true),
                        "false" => Some(false),
                        _ => None,
                    };
                }
                "cwd" => task.cwd = hnl_string_literal(&v),
                // Unknown keys are ignored — forward-compatible with
                // schema growth (same posture as `deps` spec parsing).
                _ => {}
            }
        }
        if task.run.is_empty() {
            return None;
        }
        return Some(PkgScriptValue::Task(task));
    }
    // Bare text — lenient: treat it as a command line.
    Some(PkgScriptValue::Command(t.to_string()))
}

/// Quoted-string list entries of a `[ … ]` body, in order — the shared
/// tail of [`pkg_modules`] and [`kit_libs`].
fn hnl_list_entries(body: &str) -> Vec<String> {
    body.split(',')
        .map(|s| s.trim().trim_matches('"').trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// kit.hnl v1 — pure-curation kit manifests (proposal P-004)
//
// A `kit.hnl` is a `.hnl` data literal at a `kit-*` repo's root:
//
// ```text
// { name: "web-server", description: "…", license: "MIT",
//   libs: ["serve-epoll", "http-route", "json"] }
// ```
//
// `libs` entries are lib package names — the `lib-*` repo name minus the
// `lib-` prefix — resolved by the same sibling-checkout convention
// [`resolve_pkg_dependencies`] applies to `pkg.hnl` `deps`. Like `pkg.hnl`,
// parsing is a lenient scan: a missing or malformed manifest yields empty
// results rather than an error.
// ---------------------------------------------------------------------------

/// The `name:` field of a `kit.hnl` source, when declared.
///
/// Same lenient scan as [`pkg_entry`]: comment-stripped, string-aware, and
/// `None` on a missing or non-string value.
pub fn kit_name(src: &str) -> Option<String> {
    let clean: String = src
        .lines()
        .map(|l| match l.find("--") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let at = pkg_key_at(&clean, "name")?;
    let tail = &clean[at + "name".len()..];
    let colon = tail.find(':')?;
    let value = tail[colon + 1..].trim_start();
    if !value.starts_with('"') {
        return None;
    }
    let end = value[1..].find('"')? + 1;
    Some(value[1..end].to_string())
}

/// The `libs` list of a `kit.hnl` source, in declaration order.
///
/// Empty when the table is absent or malformed — the same leniency
/// [`pkg_modules`] applies.
pub fn kit_libs(src: &str) -> Vec<String> {
    let Some(body) = pkg_list_body(src, "libs") else {
        return Vec::new();
    };
    hnl_list_entries(&body)
}

/// The sibling `lib-<name>` package directory for a lib/kit entry —
/// the local-development convention shared by [`resolve_pkg_dependencies`]
/// and [`resolve_kit_dependencies`]. Canonicalized when it exists so
/// downstream cache keys are stable.
fn sibling_lib_dir(base: &Path, name: &str) -> PathBuf {
    let dir = base.join("..").join(format!("lib-{name}"));
    std::fs::canonicalize(&dir).unwrap_or(dir)
}

/// Read `<kit_dir>/kit.hnl` and resolve its `libs` into an import-name →
/// package-directory map, closing over each package's own `pkg.hnl`
/// `deps` transitively.
///
/// This is the kit.hnl v1 loader (proposal P-004): a kit is pure curation —
/// `libs: ["js", "json"]` means "the `lib-js` and `lib-json` sibling
/// checkouts, plus whatever their `pkg.hnl` `deps` pull in" (contracts and
/// transitive backends). The result has the same shape as
/// [`resolve_dependencies`]'s map, so it drops straight into
/// `ModuleLoader::with_dependencies` or `PackageResolver::lazy`.
///
/// `kit_dir` may be the kit repo's root (the directory containing
/// `kit.hnl`) or the `kit.hnl` file itself. A missing manifest yields an
/// empty map; a malformed one is lenient, matching the `pkg.hnl` rule.
/// Names already resolved keep their first directory — kit `libs` order
/// wins over transitively discovered deps.
///
/// # Errors
///
/// Reserved for future validation; currently always `Ok`.
pub fn resolve_kit_dependencies(kit_dir: &Path) -> Result<HashMap<String, PathBuf>, PlatformError> {
    let dir = if kit_dir.is_file() {
        kit_dir.parent().unwrap_or(kit_dir)
    } else {
        kit_dir
    };
    let Ok(src) = std::fs::read_to_string(dir.join("kit.hnl")) else {
        return Ok(HashMap::new());
    };
    let mut deps: HashMap<String, PathBuf> = HashMap::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    for name in kit_libs(&src) {
        let pkg_dir = sibling_lib_dir(dir, &name);
        if let Entry::Vacant(e) = deps.entry(name) {
            e.insert(pkg_dir.clone());
            queue.push_back(pkg_dir);
        }
    }
    // Transitive closure over `pkg.hnl` `deps`: breadth-first so kit
    // `libs` (and earlier discoveries) keep precedence on a name
    // collision; `expanded` bounds the walk on dep cycles.
    let mut expanded: HashSet<PathBuf> = HashSet::new();
    while let Some(pkg_dir) = queue.pop_front() {
        if !expanded.insert(pkg_dir.clone()) {
            continue;
        }
        for (name, dep_dir) in resolve_pkg_dependencies(&pkg_dir)? {
            if let Entry::Vacant(e) = deps.entry(name) {
                e.insert(dep_dir.clone());
                queue.push_back(dep_dir);
            }
        }
    }
    Ok(deps)
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

    /// P-013 T-150: record-form dep values carry `spec`/`modules`/`atoms`.
    #[test]
    fn pkg_dep_scopes_parses_record_values() {
        let src = r#"{ deps: {
            mri: "branch:main",
            proc: { spec: "branch:main", atoms: ["proc.argv", "proc.exit"] },
            utils: { modules: ["math", "str"] },
            both: { spec: "rev:abc123", modules: ["math"], atoms: ["math.abs"] },
        } }"#;
        let deps = pkg_dep_scopes(src);
        assert_eq!(deps.len(), 4);
        assert_eq!(deps[0].name, "mri");
        assert_eq!(deps[0].spec, "branch:main");
        assert!(deps[0].scope.is_unrestricted());
        assert_eq!(deps[1].name, "proc");
        assert_eq!(deps[1].spec, "branch:main");
        assert_eq!(deps[1].scope.atoms, vec!["proc.argv", "proc.exit"]);
        assert!(deps[1].scope.modules.is_empty());
        assert_eq!(deps[2].name, "utils");
        assert_eq!(deps[2].spec, "");
        assert_eq!(deps[2].scope.modules, vec!["math", "str"]);
        assert_eq!(deps[3].spec, "rev:abc123");
        assert_eq!(deps[3].scope.modules, vec!["math"]);
        assert_eq!(deps[3].scope.atoms, vec!["math.abs"]);
        // `pkg_dep_specs` keeps working: record values contribute spec.
        assert_eq!(
            pkg_dep_specs(src),
            vec![
                ("mri".to_string(), "branch:main".to_string()),
                ("proc".to_string(), "branch:main".to_string()),
                ("utils".to_string(), String::new()),
                ("both".to_string(), "rev:abc123".to_string()),
            ]
        );
    }

    /// The scope gates: `modules` selects whole modules, `atoms` selects
    /// `module.field` exports (implying their modules).
    #[test]
    fn pkg_dep_scope_gates() {
        let atoms = PkgDepScope {
            modules: vec![],
            atoms: vec!["math.abs".to_string(), "math.e".to_string()],
        };
        assert!(atoms.allows_module("math"));
        assert!(!atoms.allows_module("str"));
        assert!(atoms.allows_field("math", "abs"));
        assert!(!atoms.allows_field("math", "sin"));
        assert!(!atoms.allows_field("str", "abs"));

        let modules = PkgDepScope {
            modules: vec!["math".to_string()],
            atoms: vec![],
        };
        assert!(modules.allows_module("math"));
        assert!(!modules.allows_module("str"));
        // Whole-module scope exposes every field of the allowed module.
        assert!(modules.allows_field("math", "anything"));
        assert!(!modules.allows_field("str", "anything"));

        // Both gates: module must be listed AND field atom-listed.
        let both = PkgDepScope {
            modules: vec!["math".to_string()],
            atoms: vec!["math.abs".to_string()],
        };
        assert!(both.allows_field("math", "abs"));
        assert!(!both.allows_field("math", "sin"));
        assert!(!both.allows_field("str", "abs"));

        assert!(PkgDepScope::default().is_unrestricted());
        assert!(PkgDepScope {
            modules: vec![],
            atoms: vec![]
        }
        .allows_field("any", "thing"));
    }

    /// `dep_spec_scope` maps an import specifier to `(scope, dep, module)`:
    /// first segment is the dep, second the module inside a multi-module
    /// package, a bare dep name scopes the dep-named primary module.
    #[test]
    fn dep_spec_scope_maps_specifiers() {
        let mut scopes = HashMap::new();
        scopes.insert(
            "utils".to_string(),
            PkgDepScope {
                modules: vec!["math".to_string()],
                atoms: vec![],
            },
        );
        scopes.insert(
            "mri".to_string(),
            PkgDepScope {
                modules: vec![],
                atoms: vec!["mri.parse".to_string()],
            },
        );
        let (scope, dep, module) = dep_spec_scope(&scopes, "utils/math").unwrap();
        assert_eq!(dep, "utils");
        assert_eq!(module, "math");
        assert!(scope.allows_module("math"));
        let (_, dep, module) = dep_spec_scope(&scopes, "utils/str").unwrap();
        assert_eq!((dep.as_str(), module.as_str()), ("utils", "str"));
        let (_, dep, module) = dep_spec_scope(&scopes, "mri").unwrap();
        assert_eq!((dep.as_str(), module.as_str()), ("mri", "mri"));
        // Unscoped dep names and relative specs resolve to no scope.
        assert!(dep_spec_scope(&scopes, "proc").is_none());
        assert!(dep_spec_scope(&scopes, "./index").is_none());
        assert!(dep_spec_scope(&scopes, "/abs/path").is_none());
    }

    /// `scope_tagged_bindings` filters an export record to the declared
    /// `module.field` surface; unscoped bindings pass through untouched.
    #[test]
    fn scope_tagged_bindings_filters_fields() {
        use handl_value::TaggedValue;
        let bindings = std::rc::Rc::new(TaggedValue::from(serde_json::json!({
            "abs": 1,
            "sin": 2
        })));
        assert!(matches!(bindings.as_ref(), TaggedValue::Obj(_)));
        let scope = PkgDepScope {
            modules: vec![],
            atoms: vec!["math.abs".to_string()],
        };
        let filtered = scope_tagged_bindings(&bindings, &scope, "math");
        let TaggedValue::Obj(m) = filtered.as_ref() else {
            panic!("expected obj");
        };
        assert!(m.contains_key("abs"));
        assert!(!m.contains_key("sin"));
        // Unrestricted scope is identity.
        let same = scope_tagged_bindings(&bindings, &PkgDepScope::default(), "math");
        assert!(std::rc::Rc::ptr_eq(&same, &bindings));
    }

    /// `pkg_scripts` reads the `scripts` table in all three value forms —
    /// string, array, and task record — with comments and nested
    /// records/arrays intact.
    #[test]
    fn pkg_scripts_reads_all_value_forms() {
        let src = r#"{ name: "curl",
  -- a comment mentioning scripts: { fake: "x" } must not shadow
  scripts: {
    gate: "hnl codegen plan --check",
    regen: "hnl codegen c specs/curl.h --output generated/curl",
    test: ["hnl test src", "cargo test -p curl-sys"],
    ci: {
      run: "bash ci.sh",
      dependsOn: ["gate", "regen"],
      cache: true,
      inputs: ["src/**", "specs/**", "pkg.hnl"],
      outputs: ["generated/**"],
      env: ["CURL_HOME"],
      cwd: "src"        -- trailing comment
    },
    "quoted-name": "echo hi && echo bye"
  }
}"#;
        let scripts = pkg_scripts(src);
        assert_eq!(scripts.len(), 5);

        assert_eq!(
            scripts[0],
            PkgScript {
                name: "gate".into(),
                value: PkgScriptValue::Command("hnl codegen plan --check".into()),
            }
        );
        assert_eq!(
            scripts[2],
            PkgScript {
                name: "test".into(),
                value: PkgScriptValue::Commands(vec![
                    "hnl test src".into(),
                    "cargo test -p curl-sys".into(),
                ]),
            }
        );
        let PkgScriptValue::Task(task) = &scripts[3].value else {
            panic!("ci must be a task");
        };
        assert_eq!(task.run, vec!["bash ci.sh".to_string()]);
        assert_eq!(
            task.depends_on,
            vec!["gate".to_string(), "regen".to_string()]
        );
        assert_eq!(task.cache, Some(true));
        assert_eq!(
            task.inputs.as_deref(),
            Some(
                &[
                    "src/**".to_string(),
                    "specs/**".to_string(),
                    "pkg.hnl".to_string()
                ][..]
            )
        );
        assert_eq!(
            task.outputs.as_deref(),
            Some(&["generated/**".to_string()][..])
        );
        assert_eq!(task.env, vec!["CURL_HOME".to_string()]);
        assert_eq!(task.cwd.as_deref(), Some("src"));
        assert_eq!(scripts[3].name, "ci");
        assert_eq!(scripts[4].name, "quoted-name");
    }

    /// Task records tolerate unknown keys, single-string list fields,
    /// and `run` as an array; entries with no usable `run` are dropped.
    #[test]
    fn pkg_scripts_task_leniency() {
        let src = r#"{ scripts: {
            a: { run: ["one", "two"], futureKey: { nested: [1, 2] } },
            b: { run: "solo", dependsOn: "a", inputs: "src/**" },
            c: { dependsOn: ["a"] },
            d: { run: [] }
        } }"#;
        let scripts = pkg_scripts(src);
        assert_eq!(scripts.len(), 2);
        let PkgScriptValue::Task(a) = &scripts[0].value else {
            panic!()
        };
        assert_eq!(a.run, vec!["one".to_string(), "two".to_string()]);
        let PkgScriptValue::Task(b) = &scripts[1].value else {
            panic!()
        };
        assert_eq!(b.run, vec!["solo".to_string()]);
        assert_eq!(b.depends_on, vec!["a".to_string()]);
        assert_eq!(b.inputs.as_deref(), Some(&["src/**".to_string()][..]));
        assert_eq!(b.cache, None);
    }

    /// String escapes and commas inside strings survive the entry split.
    #[test]
    fn pkg_scripts_string_escapes() {
        let src = r#"{ scripts: { s: "echo \"a,b\" \\ path", t: ["x, y", "z"] } }"#;
        let scripts = pkg_scripts(src);
        assert_eq!(scripts.len(), 2);
        assert_eq!(
            scripts[0].value,
            PkgScriptValue::Command("echo \"a,b\" \\ path".into())
        );
        assert_eq!(
            scripts[1].value,
            PkgScriptValue::Commands(vec!["x, y".into(), "z".into()])
        );
    }

    /// Absent or malformed `scripts` tables are leniently empty.
    #[test]
    fn pkg_scripts_is_lenient() {
        assert!(pkg_scripts("{ modules: [\"cli\"] }").is_empty());
        assert!(pkg_scripts("").is_empty());
        assert!(pkg_scripts("{ scripts: { gate: ").is_empty());
        // `scripts:` inside a string value must not shadow the real key.
        assert!(pkg_scripts("{ description: \"scripts: { x: 1 }\" }").is_empty());
    }

    /// `discover_pkg_dir` walks up to the nearest `pkg.hnl`.
    #[test]
    fn discover_pkg_dir_walks_up() {
        let root = std::env::temp_dir().join(format!("hnl-pkgdir-{}", std::process::id()));
        let proj = root.join("repo");
        let nested = proj.join("src/deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(proj.join("pkg.hnl"), "{ modules: [\"m\"] }").unwrap();

        assert_eq!(
            discover_pkg_dir(&nested).unwrap(),
            std::fs::canonicalize(&proj).unwrap()
        );
        // A file start resolves to its parent directory first.
        let file = proj.join("src/index.hnl");
        std::fs::write(&file, "").unwrap();
        assert_eq!(
            discover_pkg_dir(&file).unwrap(),
            std::fs::canonicalize(&proj).unwrap()
        );

        let bare = std::env::temp_dir().join(format!("hnl-pkgdir-bare-{}", std::process::id()));
        std::fs::create_dir_all(&bare).unwrap();
        assert!(discover_pkg_dir(&bare).is_none());

        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&bare).ok();
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
    #[test]
    fn pkg_modules_reads_the_list() {
        let src = "-- pkg\n{ deps: { }, modules: [\"vec\", \"list\"] }\n";
        assert_eq!(
            pkg_modules(src),
            vec!["vec".to_string(), "list".to_string()]
        );
        assert!(pkg_modules("{ modules: [] }").is_empty());
        assert!(pkg_modules("no manifest here").is_empty());
    }

    #[test]
    fn pkg_entry_override() {
        assert_eq!(
            pkg_entry("{ modules: [\"a\"], entry: \"src/a.hnl\" }"),
            Some("src/a.hnl".into())
        );
        assert_eq!(pkg_entry("{ modules: [\"a\"] }"), None);
    }

    /// Annotation fields (`name`/`description`/`keywords`/…) routinely
    /// mention the scanner keys in prose; only a real `key:` may match.
    #[test]
    fn pkg_scanners_ignore_annotation_prose() {
        let src = r#"{
            name: "lib-foo",
            description: "deps: see docs; entry: the start; modules listed",
            license: "MIT",
            keywords: [deps, modules, entry],
            deps: { "math-c": "branch:main" },
            modules: ["vec"],
            entry: "src/vec.hnl",
        }"#;
        assert_eq!(
            pkg_dep_specs(src),
            vec![("math-c".to_string(), "branch:main".to_string())]
        );
        assert_eq!(pkg_modules(src), vec!["vec".to_string()]);
        assert_eq!(pkg_entry(src).as_deref(), Some("src/vec.hnl"));
    }

    /// A key embedded in a longer identifier is not a match.
    #[test]
    fn pkg_scanners_require_word_boundary() {
        assert!(pkg_dep_specs("{ nodeps: { a: \"branch:main\" } }").is_empty());
        assert!(pkg_modules("{ submodules: [\"x\"] }").is_empty());
        assert_eq!(pkg_entry("{ reentry: \"src/x.hnl\" }"), None);
    }

    /// Annotations placed after the functional keys change nothing.
    #[test]
    fn pkg_annotations_after_keys_parse_identically() {
        let bare = "{ deps: { a: \"branch:main\" }, modules: [\"x\"], entry: \"src/x.hnl\" }";
        let annotated = "{ deps: { a: \"branch:main\" }, modules: [\"x\"], entry: \"src/x.hnl\", \
                         name: \"n\", description: \"deps modules entry\", keywords: [entry] }";
        assert_eq!(pkg_dep_specs(bare), pkg_dep_specs(annotated));
        assert_eq!(pkg_modules(bare), pkg_modules(annotated));
        assert_eq!(pkg_entry(bare), pkg_entry(annotated));
    }

    #[test]
    fn pkg_module_path_rules() {
        let one = vec!["pred".to_string()];
        assert_eq!(
            pkg_module_path(&one, None, "pred").as_deref(),
            Some("src/index.hnl")
        );
        assert_eq!(
            pkg_module_path(&one, Some("src/pred.hnl"), "pred").as_deref(),
            Some("src/pred.hnl")
        );
        let many = vec!["vec".to_string(), "list".to_string()];
        assert_eq!(
            pkg_module_path(&many, None, "list").as_deref(),
            Some("src/list/index.hnl")
        );
        assert_eq!(pkg_module_path(&many, None, "nope"), None);
    }

    #[test]
    fn dep_spec_ref_parses_kinds() {
        assert_eq!(dep_spec_ref("branch:main"), ("branch", "main".to_string()));
        assert_eq!(dep_spec_ref("rev:abc123"), ("rev", "abc123".to_string()));
        assert_eq!(dep_spec_ref("tag:v1"), ("tag", "v1".to_string()));
        assert_eq!(dep_spec_ref(""), ("branch", "main".to_string()));
    }

    #[test]
    fn dep_repo_candidates_are_ordered() {
        assert_eq!(
            dep_repo_candidates("pred"),
            vec![
                "lib-pred",
                "lib-pred-handl",
                "lib-pred-c",
                "lib-pred-native"
            ]
        );
    }

    // -- kit.hnl v1 ---------------------------------------------------------

    /// `kit_name`/`kit_libs` read the kit.hnl v1 fields — and ignore the
    /// annotation prose the real manifests carry (`description` mentions
    /// "libs"-adjacent words; comments precede the record).
    #[test]
    fn kit_manifest_fields_parse() {
        let src = "-- kit-test: kit.hnl v1 curation manifest\n\
                   { name: \"kit-test\",\n  \
                   \x20 description: \"libs: see proposal; name: not a field\",\n  \
                   \x20 license: \"MIT\",\n  \
                   \x20 libs: [\"test\", \"rand\", \"rand-stub\"], }\n";
        assert_eq!(kit_name(src).as_deref(), Some("kit-test"));
        assert_eq!(
            kit_libs(src),
            vec![
                "test".to_string(),
                "rand".to_string(),
                "rand-stub".to_string()
            ]
        );
        assert_eq!(kit_name("{ libs: [] }"), None);
        assert!(kit_libs("no manifest here").is_empty());
        assert!(kit_libs("{ libs: [").is_empty());
    }

    /// A `name:` embedded in a longer identifier is not a match.
    #[test]
    fn kit_name_requires_word_boundary() {
        assert_eq!(kit_name("{ username: \"x\" }"), None);
        assert_eq!(kit_name("{ name: 42 }"), None);
    }

    /// `resolve_kit_dependencies` maps each `libs` entry to its sibling
    /// `lib-<name>` checkout and closes over `pkg.hnl` `deps`
    /// transitively — the kit.hnl v1 loader contract.
    #[test]
    fn resolve_kit_dependencies_maps_libs_and_transitive_deps() {
        let root = std::env::temp_dir().join(format!("hnl-kitdeps-{}", std::process::id()));
        let kit = root.join("kit-demo");
        let lib_a = root.join("lib-a");
        let lib_b = root.join("lib-b");
        let lib_c = root.join("lib-c");
        for dir in [&kit, &lib_a, &lib_b, &lib_c] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(
            kit.join("kit.hnl"),
            "{ name: \"kit-demo\", license: \"MIT\", libs: [\"a\", \"b\"] }\n",
        )
        .unwrap();
        // `a` pulls contract `c` through its own pkg.hnl deps; `b` is a
        // leaf. The kit also lists `c`'s dep explicitly? No — closure only.
        std::fs::write(
            lib_a.join("pkg.hnl"),
            "{ modules: [\"a\"], deps: { c: \"branch:main\" } }\n",
        )
        .unwrap();
        std::fs::write(lib_b.join("pkg.hnl"), "{ modules: [\"b\"] }\n").unwrap();
        std::fs::write(lib_c.join("pkg.hnl"), "{ modules: [\"c\"] }\n").unwrap();

        let deps = resolve_kit_dependencies(&kit).unwrap();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps["a"], std::fs::canonicalize(&lib_a).unwrap());
        assert_eq!(deps["b"], std::fs::canonicalize(&lib_b).unwrap());
        assert_eq!(deps["c"], std::fs::canonicalize(&lib_c).unwrap());

        // The kit.hnl file path itself resolves the same way.
        let deps2 = resolve_kit_dependencies(&kit.join("kit.hnl")).unwrap();
        assert_eq!(deps2, deps);

        std::fs::remove_dir_all(&root).ok();
    }

    /// Kit `libs` order wins on a name collision with a transitively
    /// discovered dep, and a missing `kit.hnl`/`lib-*` dir is lenient.
    #[test]
    fn resolve_kit_dependencies_is_lenient_and_ordered() {
        let root = std::env::temp_dir().join(format!("hnl-kitdeps-bad-{}", std::process::id()));
        let empty = root.join("kit-empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(resolve_kit_dependencies(&empty).unwrap().is_empty());

        // `a` exists and deps on `x`; the kit lists `x` *first* — the kit's
        // own `lib-x` entry must win over `a`'s transitive discovery even
        // though both resolve to the same sibling convention here. Use a
        // dep name the kit also lists to prove first-wins: kit lists
        // ["x", "a"], `a` deps on `x` — `x` maps once.
        let kit = root.join("kit-ord");
        let lib_x = root.join("lib-x");
        let lib_a = root.join("lib-a");
        for dir in [&kit, &lib_x, &lib_a] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(kit.join("kit.hnl"), "{ libs: [\"x\", \"a\"] }\n").unwrap();
        std::fs::write(
            lib_a.join("pkg.hnl"),
            "{ modules: [\"a\"], deps: { x: \"branch:main\" } }\n",
        )
        .unwrap();
        std::fs::write(lib_x.join("pkg.hnl"), "{ modules: [\"x\"] }\n").unwrap();

        let deps = resolve_kit_dependencies(&kit).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps["x"], std::fs::canonicalize(&lib_x).unwrap());
        assert_eq!(deps["a"], std::fs::canonicalize(&lib_a).unwrap());

        // A `libs` entry with no sibling checkout still maps (lenient —
        // the absent dir fails at import time, naming the slot).
        let ghost = root.join("kit-ghost");
        std::fs::create_dir_all(&ghost).unwrap();
        std::fs::write(ghost.join("kit.hnl"), "{ libs: [\"missing\"] }\n").unwrap();
        let deps = resolve_kit_dependencies(&ghost).unwrap();
        assert_eq!(deps.len(), 1);
        assert!(deps.contains_key("missing"));

        std::fs::remove_dir_all(&root).ok();
    }

    // -- T-122: `.handl.toml` `kit =` discovery --------------------------------

    /// `kit = "<name-or-path>"` parses alongside the rest of the manifest
    /// but contributes no host-kit entries — it is the eval-kit
    /// selection key, a different namespace from `[[kit]]`.
    #[test]
    fn host_config_accepts_kit_string_key() {
        let config =
            HostConfig::from_toml("kit = \"../kit-web\"\n[dependencies]\nx = \"vendor/x\"\n")
                .unwrap();
        assert!(config.kits.is_empty());
        assert_eq!(config.dependencies["x"].path.as_deref(), Some("vendor/x"));

        // `[[kit]]` tables still populate `kits`.
        let config = HostConfig::from_toml("[[kit]]\ninclude = \"handl-kit-canonical\"\n").unwrap();
        assert_eq!(config.kits.len(), 1);
        assert_eq!(config.kits[0].include, "handl-kit-canonical");

        // A non-string, non-table `kit` is still a manifest error.
        assert!(HostConfig::from_toml("kit = 42\n").is_err());
    }

    /// Nearest `.handl.toml` with a `kit =` key wins; a manifest without
    /// the key does not stop the walk; `start` may be the file itself.
    #[test]
    fn discover_kit_spec_nearest_manifest_wins() {
        let root = std::env::temp_dir().join(format!("hnl-kitdisc-{}", std::process::id()));
        let outer = root.join("outer");
        let inner = outer.join("inner");
        let deep = inner.join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        let file = deep.join("main.hnl");
        std::fs::write(&file, "1").unwrap();

        // Outer declares a kit; inner's manifest has none — outer wins.
        std::fs::write(outer.join(".handl.toml"), "kit = \"kit-outer\"\n").unwrap();
        std::fs::write(inner.join(".handl.toml"), "[dependencies]\n").unwrap();

        let found = discover_kit_spec_in(&file, None).unwrap();
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&outer).unwrap());
        assert_eq!(found.spec, "kit-outer");

        // Inner gains a `kit` key — now it is the nearest and wins
        // (replace, never union: the outer spec is not consulted).
        std::fs::write(inner.join(".handl.toml"), "kit = \"../kit-inner\"\n").unwrap();
        let found = discover_kit_spec_in(&file, None).unwrap();
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&inner).unwrap());
        assert_eq!(found.spec, "../kit-inner");

        std::fs::remove_dir_all(&root).ok();
    }

    /// `[[kit]]` host-kit tables are not the eval-kit key: a manifest
    /// carrying only `[[kit]]` does not satisfy discovery.
    #[test]
    fn discover_kit_spec_ignores_host_kit_tables() {
        let root = std::env::temp_dir().join(format!("hnl-kitdisc-host-{}", std::process::id()));
        let proj = root.join("proj");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(
            proj.join(".handl.toml"),
            "[[kit]]\ninclude = \"handl-kit-canonical\"\n",
        )
        .unwrap();

        assert!(discover_kit_spec_in(&proj, None).is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    /// No project manifest with `kit` → the explicit global manifest is
    /// the fallback; a project `kit` shadows it entirely.
    #[test]
    fn discover_kit_spec_falls_back_to_global() {
        let root = std::env::temp_dir().join(format!("hnl-kitdisc-glob-{}", std::process::id()));
        let proj = root.join("proj");
        let cfg = root.join("config");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&cfg).unwrap();
        let global = cfg.join(".handl.toml");
        std::fs::write(&global, "kit = \"kit-global\"\n").unwrap();

        // Bare project dir: global fallback applies, anchored at the
        // global manifest's own directory.
        let found = discover_kit_spec_in(&proj, Some(&global)).unwrap();
        assert_eq!(found.spec, "kit-global");
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&cfg).unwrap());

        // A project `kit` replaces the global one — no union.
        std::fs::write(proj.join(".handl.toml"), "kit = \"kit-proj\"\n").unwrap();
        let found = discover_kit_spec_in(&proj, Some(&global)).unwrap();
        assert_eq!(found.spec, "kit-proj");
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&proj).unwrap());

        // Global manifest without a `kit` key yields nothing.
        std::fs::remove_file(proj.join(".handl.toml")).unwrap();
        std::fs::write(&global, "[dependencies]\n").unwrap();
        assert!(discover_kit_spec_in(&proj, Some(&global)).is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    // -- `extends` inheritance ------------------------------------------------

    /// A manifest without `kit` but with `extends` hands discovery to
    /// the named manifest: a relative path to another `.handl.toml`
    /// (or its directory) resolves against the declaring manifest's
    /// dir, and the chained manifest's `kit` applies — anchored at the
    /// chained manifest's own directory.
    #[test]
    fn discover_kit_spec_follows_extends_chain() {
        let root = std::env::temp_dir().join(format!("hnl-kitext-{}", std::process::id()));
        let proj = root.join("proj");
        let base = root.join("base");
        std::fs::create_dir_all(proj.join("src")).unwrap();
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join(".handl.toml"), "kit = \"../kit-base\"\n").unwrap();
        // Path form naming the manifest file directly.
        std::fs::write(
            proj.join(".handl.toml"),
            "extends = \"../base/.handl.toml\"\n",
        )
        .unwrap();

        let found = discover_kit_spec_in_checked(&proj.join("src"), None)
            .unwrap()
            .unwrap();
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&base).unwrap());
        assert_eq!(found.spec, "../kit-base");
        assert_eq!(found.extends, None);

        // Directory form (`extends = "../base"`) resolves the same way.
        std::fs::write(proj.join(".handl.toml"), "extends = \"../base\"\n").unwrap();
        let found = discover_kit_spec_in_checked(&proj, None).unwrap().unwrap();
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&base).unwrap());
        assert_eq!(found.spec, "../kit-base");

        // Multi-hop: proj → mid → base; mid declares no kit of its own.
        let mid = root.join("mid");
        std::fs::create_dir_all(&mid).unwrap();
        std::fs::write(mid.join(".handl.toml"), "extends = \"../base\"\n").unwrap();
        std::fs::write(proj.join(".handl.toml"), "extends = \"../mid\"\n").unwrap();
        let found = discover_kit_spec_in_checked(&proj, None).unwrap().unwrap();
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&base).unwrap());
        assert_eq!(found.spec, "../kit-base");

        // A chain that ends without a `kit` falls through to the normal
        // ancestor walk — an outer manifest's `kit` still applies.
        let outer = root.join("outer");
        std::fs::create_dir_all(outer.join("inner")).unwrap();
        std::fs::write(outer.join(".handl.toml"), "kit = \"kit-outer\"\n").unwrap();
        std::fs::write(
            outer.join("inner").join(".handl.toml"),
            "extends = \"../../base-empty\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("base-empty")).unwrap();
        std::fs::write(
            root.join("base-empty").join(".handl.toml"),
            "[dependencies]\n",
        )
        .unwrap();
        let found = discover_kit_spec_in_checked(&outer.join("inner"), None)
            .unwrap()
            .unwrap();
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&outer).unwrap());
        assert_eq!(found.spec, "kit-outer");

        std::fs::remove_dir_all(&root).ok();
    }

    /// `extends` accepts a bare name resolved like a kit spec: probe
    /// `<manifest_dir>/kit-<name>` then the parent directory.
    #[test]
    fn discover_kit_spec_extends_bare_name() {
        let root = std::env::temp_dir().join(format!("hnl-kitext-bare-{}", std::process::id()));
        let proj = root.join("proj");
        let kit_manifest = root.join("kit-shared");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&kit_manifest).unwrap();
        std::fs::write(kit_manifest.join(".handl.toml"), "kit = \"kit-real\"\n").unwrap();
        std::fs::write(proj.join(".handl.toml"), "extends = \"shared\"\n").unwrap();

        // `<proj>/kit-shared` does not exist; the parent probe
        // `<root>/kit-shared` does.
        let found = discover_kit_spec_in_checked(&proj, None).unwrap().unwrap();
        assert_eq!(found.spec, "kit-real");
        assert_eq!(
            found.manifest_dir,
            std::fs::canonicalize(&kit_manifest).unwrap()
        );

        // A `kit-<name>` dir inside the manifest dir wins first.
        let inner = proj.join("kit-inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join(".handl.toml"), "kit = \"kit-inner-real\"\n").unwrap();
        std::fs::write(proj.join(".handl.toml"), "extends = \"inner\"\n").unwrap();
        let found = discover_kit_spec_in_checked(&proj, None).unwrap().unwrap();
        assert_eq!(found.spec, "kit-inner-real");
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&inner).unwrap());

        std::fs::remove_dir_all(&root).ok();
    }

    /// A manifest declaring both `kit` and `extends`: `kit` wins for
    /// kit selection (the extends target's `kit` is never consulted)
    /// and the resolved `extends` link is recorded on the result.
    #[test]
    fn discover_kit_spec_kit_wins_over_extends() {
        let root = std::env::temp_dir().join(format!("hnl-kitext-both-{}", std::process::id()));
        let proj = root.join("proj");
        let base = root.join("base");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join(".handl.toml"), "kit = \"kit-base\"\n").unwrap();
        std::fs::write(
            proj.join(".handl.toml"),
            "kit = \"kit-proj\"\nextends = \"../base\"\n",
        )
        .unwrap();

        let found = discover_kit_spec_in_checked(&proj, None).unwrap().unwrap();
        assert_eq!(found.manifest_dir, std::fs::canonicalize(&proj).unwrap());
        assert_eq!(found.spec, "kit-proj");
        assert_eq!(
            found.extends,
            Some(std::fs::canonicalize(base.join(".handl.toml")).unwrap())
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// `extends` cycles and over-deep chains are hard errors on the
    /// checked entry point (the lenient wrapper reports `None`).
    #[test]
    fn discover_kit_spec_extends_cycle_and_depth() {
        let root = std::env::temp_dir().join(format!("hnl-kitext-cyc-{}", std::process::id()));
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join(".handl.toml"), "extends = \"../b\"\n").unwrap();
        std::fs::write(b.join(".handl.toml"), "extends = \"../a\"\n").unwrap();

        let err = discover_kit_spec_in_checked(&a, None).unwrap_err();
        assert!(format!("{err}").contains("extends cycle"), "{err}");
        // Lenient wrapper: the broken chain reads as "no kit found".
        assert!(discover_kit_spec_in(&a, None).is_none());

        // Self-cycle is caught too.
        std::fs::write(a.join(".handl.toml"), "extends = \".\"\n").unwrap();
        // `.` has no separator → bare-name probe misses → dangling error.
        let err = discover_kit_spec_in_checked(&a, None).unwrap_err();
        assert!(format!("{err}").contains("extends"), "{err}");
        std::fs::write(a.join(".handl.toml"), "extends = \"./.handl.toml\"\n").unwrap();
        let err = discover_kit_spec_in_checked(&a, None).unwrap_err();
        assert!(format!("{err}").contains("extends cycle"), "{err}");

        // Depth cap: a linear chain of MAX+2 manifests, none with `kit`.
        let chain = root.join("chain");
        for i in 0..10 {
            let dir = chain.join(format!("m{i}"));
            std::fs::create_dir_all(&dir).unwrap();
            let next = if i == 9 {
                "dead".to_string()
            } else {
                format!("../m{}", i + 1)
            };
            std::fs::write(dir.join(".handl.toml"), format!("extends = \"{next}\"\n")).unwrap();
        }
        std::fs::create_dir_all(chain.join("dead")).unwrap();
        std::fs::write(chain.join("dead").join(".handl.toml"), "[dependencies]\n").unwrap();
        let err = discover_kit_spec_in_checked(&chain.join("m0"), None).unwrap_err();
        assert!(format!("{err}").contains("depth"), "{err}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// A dangling `extends` (no resolvable manifest) is an error on the
    /// checked entry point, not a silent skip.
    #[test]
    fn discover_kit_spec_extends_dangling_is_error() {
        let root = std::env::temp_dir().join(format!("hnl-kitext-dangle-{}", std::process::id()));
        let proj = root.join("proj");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join(".handl.toml"), "extends = \"../nowhere\"\n").unwrap();

        let err = discover_kit_spec_in_checked(&proj, None).unwrap_err();
        assert!(format!("{err}").contains("extends"), "{err}");
        assert!(discover_kit_spec_in(&proj, None).is_none());

        std::fs::remove_dir_all(&root).ok();
    }
}
