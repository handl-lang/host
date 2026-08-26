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
/// The manifest lists the token, AST-shape, and grammar-rule plugins that
/// the kit wires together into a single `CanonicalKind` token stream and
/// a shared `KitNode` AST.
pub fn canonical_manifest() -> KitManifest {
    KitManifest {
        id: "canonical".to_string(),
        version: "0.1.0".to_string(),
        plugins: vec![
            // Token plugins
            "plugin-token-number".to_string(),
            "plugin-token-string".to_string(),
            "plugin-token-identifier".to_string(),
            "plugin-token-bool".to_string(),
            "plugin-token-null".to_string(),
            "plugin-token-comment".to_string(),
            "plugin-token-delimiter".to_string(),
            "plugin-token-operator".to_string(),
            "plugin-token-keyword-as".to_string(),
            "plugin-token-keyword-handle".to_string(),
            // AST shape plugins
            "plugin-ast-var".to_string(),
            "plugin-ast-call".to_string(),
            "plugin-ast-apply".to_string(),
            "plugin-ast-index".to_string(),
            "plugin-ast-pipe".to_string(),
            "plugin-ast-array".to_string(),
            "plugin-ast-object".to_string(),
            "plugin-ast-let".to_string(),
            "plugin-ast-assign".to_string(),
            "plugin-ast-match".to_string(),
            "plugin-ast-handle".to_string(),
            // Grammar rule plugins
            "plugin-grammar-call".to_string(),
            "plugin-grammar-apply".to_string(),
            "plugin-grammar-index".to_string(),
            "plugin-grammar-pipe".to_string(),
            "plugin-grammar-let".to_string(),
            "plugin-grammar-assign".to_string(),
            "plugin-grammar-match".to_string(),
            "plugin-grammar-handle".to_string(),
            "plugin-grammar-array".to_string(),
            "plugin-grammar-object".to_string(),
        ],
    }
}
