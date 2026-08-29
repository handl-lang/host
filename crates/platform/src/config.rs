//! Host configuration and appliers.
//!
//! This module provides the in-memory description of which kits and plugins a
//! host should load, what to exclude, and which option overrides to apply. It
//! also defines the `HostApplier` trait and a `.handl.toml` backed applier.

use std::collections::HashMap;

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
}
