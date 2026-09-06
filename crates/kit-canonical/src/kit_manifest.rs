use std::path::Path;

use handl_platform::PlatformError;
use serde::{Deserialize, Serialize};

/// A manifest describing the plugins bundled in a kit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitManifest {
    /// The kit identifier.
    pub id: String,
    /// The kit version.
    pub version: String,
    /// The list of plugin ids included in this kit.
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
            toml::from_str(&content).map_err(|e| PlatformError::InvalidMetadata(e.to_string()))
        }
    }
}

/// Returns the default manifest for the canonical kit.
///
/// The manifest lists the token, AST-shape, and grammar-rule plugins that
/// the kit wires together into a single `LexKind` token stream and
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
            "plugin-grammar-primary-value".to_string(),
            "plugin-grammar-primary-identifier".to_string(),
            "plugin-grammar-call".to_string(),
            "plugin-grammar-apply".to_string(),
            // Eval behavior plugins
            "plugin-eval-hello".to_string(),
            "plugin-eval-json".to_string(),
            "plugin-eval-core".to_string(),
            "plugin-eval-value".to_string(),
            "plugin-eval-call".to_string(),
            "plugin-eval-array".to_string(),
            "plugin-eval-object".to_string(),
            "plugin-eval-index".to_string(),
            "plugin-eval-field-access".to_string(),
            "plugin-eval-handle".to_string(),
            "plugin-eval-assign".to_string(),
            "plugin-eval-function".to_string(),
            "plugin-eval-match".to_string(),
            // Module and exec-control plugins
            "plugin-module".to_string(),
            "plugin-exec-max-depth".to_string(),
            "plugin-exec-max-calls".to_string(),
            "plugin-grammar-index".to_string(),
            "plugin-grammar-pipe".to_string(),
            "plugin-grammar-cmp".to_string(),
            "plugin-grammar-infix-flow".to_string(),
            "plugin-grammar-infix-fanout".to_string(),
            "plugin-grammar-arith".to_string(),
            "plugin-grammar-function-literal".to_string(),
            "plugin-grammar-template".to_string(),
            "plugin-grammar-assign".to_string(),
            "plugin-grammar-type-annot".to_string(),
            "plugin-grammar-match".to_string(),
            "plugin-grammar-handle".to_string(),
            "plugin-grammar-array".to_string(),
            "plugin-grammar-object".to_string(),
            // Statement grammar plugins
            "plugin-grammar-stmt-import".to_string(),
            "plugin-grammar-stmt-export".to_string(),
            "plugin-grammar-stmt-module".to_string(),
            // Lint behavior plugin
            "plugin-diag-expression".to_string(),
            // Build and compile behavior plugins (placeholders)
            // "plugin-build-core".to_string(),
            // "plugin-compile-core".to_string(),
        ],
    }
}
