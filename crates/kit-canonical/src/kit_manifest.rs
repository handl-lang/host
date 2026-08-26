use std::path::Path;

use handl_platform::PlatformError;
use serde::{Deserialize, Serialize};

/// A manifest describing the plugins bundled in a kit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitManifest {
    pub id: String,
    pub version: String,
    pub plugins: Vec<String>,
}

impl KitManifest {
    /// Load a `KitManifest` from a TOML or JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PlatformError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            PlatformError::InvalidMetadata(format!(
                "failed to read kit manifest at {}: {}",
                path.display(),
                e
            ))
        })?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "json" {
            serde_json::from_str(&content)
                .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))
        } else {
            toml::from_str(&content)
                .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))
        }
    }
}

/// Returns the default manifest for the canonical kit.
///
/// This manifest registers the three core canonical plugins created in
/// ADR-0001 step 4: the number token plugin, the call AST shape plugin,
/// and the call grammar-rule plugin.
pub fn canonical_manifest() -> KitManifest {
    KitManifest {
        id: "canonical".to_string(),
        version: "0.1.0".to_string(),
        plugins: vec![
            "plugin-token-number".to_string(),
            "plugin-ast-call".to_string(),
            "plugin-grammar-call".to_string(),
        ],
    }
}
