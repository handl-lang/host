//! Runtime configuration and appliers.
//!
//! This module provides the in-memory description of which kits and plugins a
//! host should load, what to exclude, and which option overrides to apply. It
//! also defines the `HostApplier` trait and a `.handl.toml` backed applier.

use std::collections::HashMap;
use std::path::Path;

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

/// A project dependency declared in `[dependencies]` of `.handl.toml`.
///
/// Phase 9.5 package management supports **vendored / path dependencies
/// only** — a dependency names a local package directory containing `.hnl`
/// sources. The name is the import specifier's first path segment:
/// `import "arrows/transform/map.hnl"` resolves inside the `arrows` dependency's
/// directory. Registry/content-addressed resolution is future work
/// (ADR-0010).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencySpec {
    /// Path to the package root, relative to the manifest directory
    /// (or absolute).
    pub path: String,
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
            dependencies: raw
                .dependencies
                .into_iter()
                .map(|(k, v)| {
                    let path = match v {
                        RawDependency::Path(p) => p,
                        RawDependency::Spec { path } => path,
                    };
                    (k, DependencySpec { path })
                })
                .collect(),
        })
    }
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

/// A `[dependencies]` entry: either a bare path string (`arrows =
/// "vendor/arrows"`) or a table with a `path` key (`arrows = { path =
/// "vendor/arrows" }`). The table form leaves room for future specifiers
/// (hashes, tags) without changing the schema.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawDependency {
    Path(String),
    Spec { path: String },
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

/// Read `<manifest_dir>/.handl.toml` and resolve its `[dependencies]` table
/// into an import-name → absolute package-directory map.
///
/// Dependency paths are relative to the manifest directory; absolute paths
/// are honored as-is. A missing manifest yields an empty map (no
/// dependencies). A present-but-unparseable manifest is an error — silent
/// fallback would make `import` failures look like missing files rather than
/// a broken manifest.
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
    if !manifest_path.is_file() {
        return Ok(HashMap::new());
    }
    let src = std::fs::read_to_string(&manifest_path).map_err(|e| {
        PlatformError::InvalidMetadata(format!(
            "cannot read {}: {e}",
            manifest_path.display()
        ))
    })?;
    let config = HostConfig::from_toml(&src).map_err(|e| {
        PlatformError::InvalidMetadata(format!("invalid {}: {e}", manifest_path.display()))
    })?;
    Ok(config
        .dependencies
        .into_iter()
        .map(|(name, spec)| {
            let p = std::path::PathBuf::from(&spec.path);
            let abs = if p.is_absolute() {
                p
            } else {
                manifest_dir.join(p)
            };
            // Canonicalize when possible so the module cache keys are
            // stable; fall back to the joined path otherwise.
            let abs = std::fs::canonicalize(&abs).unwrap_or(abs);
            (name, abs)
        })
        .collect())
}

/// Discover the project manifest by walking up from `start` and resolve its
/// `[dependencies]` table. Equivalent to `discover_manifest_dir` +
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
        assert_eq!(config.dependencies["arrows"].path, "vendor/arrows");
        assert_eq!(config.dependencies["utils"].path, "../shared/utils");
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
}
