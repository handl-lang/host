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
pub fn canonical_manifest() -> KitManifest {
    KitManifest {
        id: "canonical".to_string(),
        version: "0.1.0".to_string(),
        plugins: vec![
            "operators".to_string(),
            "bytes".to_string(),
            "booleans".to_string(),
            "null".to_string(),
            "keywords".to_string(),
            "strings".to_string(),
            "numbers".to_string(),
            "comment-tokens".to_string(),
            "delimiters".to_string(),
            "identifiers".to_string(),
            "primary-values".to_string(),
            "primary-identifiers".to_string(),
            "primary-arrays".to_string(),
            "primary-objects".to_string(),
            "primary-parens".to_string(),
            "primary-not".to_string(),
            "primary-typeof".to_string(),
            "primary-handle".to_string(),
            "primary-match".to_string(),
            "postfix-calls".to_string(),
            "postfix-index".to_string(),
            "postfix-field".to_string(),
            "postfix-mood".to_string(),
            "function-literals".to_string(),
            "infix-flow".to_string(),
            "infix-fanout".to_string(),
            "infix-pipe".to_string(),
            "infix-ternary".to_string(),
            "infix-assign".to_string(),
            "stmt-let".to_string(),
            "stmt-const".to_string(),
            "stmt-import".to_string(),
            "stmt-export".to_string(),
            "stmt-module".to_string(),
            "typecheck-core".to_string(),
            "vm-rust".to_string(),
        ],
    }
}
