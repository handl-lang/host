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
///   `host:owner/repo` shorthand) with an optional `rev`/`branch`/`tag`
///   pin. The loader does not fetch: at resolve time the name maps to
///   the package's slot under [`package_cache_dir`], which a `hnl`-side
///   fetch hook (ADR-0010's registry half — the follow-up seam)
///   materializes. A `[patch]` entry of the same name overrides the
///   source with a local checkout — the Cargo `[patch]` dev-link story
///   (`scripts/dev-link.sh`), so a *committed* manifest can name the
///   repo portably while a local machine resolves the sibling dir.
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
/// slot a `hnl`-side package fetch materializes the package's `.hnl`
/// tree into (ADR-0010's registry half — the follow-up seam; today
/// nothing populates it).
///
/// Location: `$HANDL_PKG_DIR` when set, otherwise `~/.handl/pkg`.
/// Keeping the unresolved name in the dep map — pointing at its cache
/// slot — means an `import` of an unfetched package fails naming the
/// directory a fetch would populate, instead of silently falling through
/// to relative-path resolution.
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
            // Canonicalize when possible so the module cache keys are
            // stable; fall back to the joined path otherwise.
            std::fs::canonicalize(&abs).unwrap_or(abs)
        }
        None => pkg_dir.join(name),
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
/// resolves to its [`package_cache_dir`] slot, which a package fetch
/// (ADR-0010 follow-up) populates. The `[patch]` table is the
/// consumer-supplied local override for the same names — the Cargo
/// `[patch]`/dev-link story (`scripts/dev-link.sh`):
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
    let manifest_path = manifest_dir.join(".handl.toml");
    let mut deps: HashMap<String, PathBuf> = if manifest_path.is_file() {
        let src = std::fs::read_to_string(&manifest_path).map_err(|e| {
            PlatformError::InvalidMetadata(format!(
                "cannot read {}: {e}",
                manifest_path.display()
            ))
        })?;
        let config = HostConfig::from_toml(&src).map_err(|e| {
            PlatformError::InvalidMetadata(format!("invalid {}: {e}", manifest_path.display()))
        })?;
        let pkg_dir = package_cache_dir();
        let HostConfig {
            dependencies,
            patch,
            ..
        } = config;
        let mut deps: HashMap<String, PathBuf> = dependencies
            .into_iter()
            .map(|(name, spec)| {
                let dir = dep_dir(manifest_dir, &pkg_dir, &name, &spec);
                (name, dir)
            })
            .collect();
        // `[patch]` — the local-override table: a patch whose named
        // directory exists wins over the `[dependencies]` source for
        // that name; a missing patch dir is inert.
        for (name, spec) in patch {
            let dir = dep_dir(manifest_dir, &pkg_dir, &name, &spec);
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
    let Some(body) = pkg_deps_body(&src) else {
        return Ok(HashMap::new());
    };
    let mut out = HashMap::new();
    for (name, _spec) in pkg_dep_entries(&body) {
        // Local-dev convention: the dep lives in the sibling `lib-<name>` repo.
        let dir = manifest_dir.join("..").join(format!("lib-{name}"));
        let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        out.insert(name, dir);
    }
    Ok(out)
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
                "unpatched-git = { git = \"github:handl-lang/lib-upstream\" }\n",
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
    /// even though nothing has fetched it yet — an `import` of the name
    /// fails naming the slot a fetch would populate.
    #[test]
    fn unpatched_git_dependency_maps_to_the_package_cache() {
        let root = std::env::temp_dir().join(format!("hnl-gitdep-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(".handl.toml"),
            "[dependencies]\nws-c = { git = \"https://github.com/handl-lang/c\", rev = \"abc123\" }\n",
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
        assert_eq!(
            deps["ws-c"],
            std::fs::canonicalize(&dep_dir).unwrap()
        );
        assert_eq!(discover_manifest_dir(&nested).unwrap(), std::fs::canonicalize(&root).unwrap());

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
        assert_eq!(
            deps["math-c"],
            std::fs::canonicalize(&math_c_dep).unwrap()
        );

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
}
