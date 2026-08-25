use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;
use crate::manifest::PluginManifest;
use crate::resolve::{CapabilitySelector, ResolutionError, ResolvedComposition};

/// A manifest describing a kit: a named composition of language plugins.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct KitManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub plugins: Vec<String>,
    pub target: Option<String>,
    #[serde(default)]
    pub selectors: Vec<CapabilitySelector>,
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
        if ext == "toml" {
            toml::from_str(&content)
                .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))
        } else if ext == "json" {
            serde_json::from_str(&content)
                .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))
        } else {
            toml::from_str(&content)
                .or_else(|_| serde_json::from_str(&content))
                .map_err(|e| PlatformError::InvalidMetadata(e.to_string()))
        }
    }
}

/// A kit is a loaded manifest plus the concrete plugin manifests that are
/// available for resolution in the host.
#[derive(Debug, Clone, Default)]
pub struct Kit {
    pub manifest: KitManifest,
    pub available_plugins: Vec<PluginManifest>,
}

impl Kit {
    /// Resolve the plugin composition for this kit.
    pub fn resolve(&self) -> Result<ResolvedComposition, ResolutionError> {
        let selected: HashSet<_> = self.manifest.plugins.iter().cloned().collect();
        let manifests: Vec<_> = self
            .available_plugins
            .iter()
            .filter(|p| selected.contains(&p.id))
            .cloned()
            .collect();

        crate::resolve::resolve(
            &manifests,
            &self.manifest.selectors,
            self.manifest.target.as_deref(),
        )
    }
}

#[derive(Default)]
pub struct KitRegistry {
    kits: Vec<Kit>,
}

impl KitRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, kit: Kit) {
        self.kits.push(kit);
    }

    pub fn resolve(&self, name: &str) -> Option<&Kit> {
        self.kits.iter().find(|k| k.manifest.name == name)
    }

    pub fn all(&self) -> &[Kit] {
        &self.kits
    }

    pub fn remove(&mut self, name: &str) -> Result<(), PlatformError> {
        let len_before = self.kits.len();
        self.kits.retain(|k| k.manifest.name != name);
        if self.kits.len() == len_before {
            Err(PlatformError::InvalidMetadata(format!("kit not found: {name}")))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit_manifest() -> KitManifest {
        KitManifest {
            id: "core".into(),
            name: "core".into(),
            plugins: vec!["fmt".into(), "lint".into()],
            target: None,
            selectors: Vec::new(),
        }
    }

    #[test]
    fn kit_manifest_construction() {
        let m = kit_manifest();
        assert_eq!(m.id, "core");
        assert_eq!(m.name, "core");
        assert_eq!(m.plugins, vec!["fmt", "lint"]);
    }

    #[test]
    fn kit_registry_register_resolve_and_all() {
        let mut reg = KitRegistry::new();
        reg.register(Kit {
            manifest: kit_manifest(),
            available_plugins: Vec::new(),
        });

        assert_eq!(reg.all().len(), 1);
        assert_eq!(reg.resolve("core").unwrap().manifest.plugins.len(), 2);
        assert!(reg.resolve("missing").is_none());
    }
}
