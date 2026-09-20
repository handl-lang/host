//! `.sdkgen.yaml` configuration for the WebIDL backend.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Per-library WebIDL SDK generation configuration.
#[derive(Debug, Default, Deserialize)]
pub struct WebIdlConfig {
    /// Hand-written doc comments, keyed by symbol name (`Event`),
    /// record/member path (`Event.bubbles`), or constant (`ShadowRootMode`).
    /// Overrides beat both spec comments and MDN lookup.
    #[serde(default)]
    pub docs: HashMap<String, String>,
    /// Set `false` to disable external doc resolution (MDN web fetches).
    /// Absent = enabled.
    pub external_docs: Option<bool>,
    /// Supplementary spec files (paths relative to this config file) that
    /// contribute mixin definitions for cross-spec `X includes Y`
    /// resolution — e.g. encoding.idl's `GenericTransformStream` lives in
    /// streams.idl. Their interfaces and records are not emitted.
    #[serde(default)]
    pub includes: Vec<String>,
}

impl WebIdlConfig {
    /// Load a `.sdkgen.yaml` file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config {}: {}", path.display(), e))?;
        serde_yaml::from_str(&s).map_err(|e| format!("failed to parse config: {e}"))
    }
}
