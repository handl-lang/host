//! Kit manifests and registries.

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
    /// Unique kit identifier.
    pub id: String,
    /// Human-readable kit name.
    pub name: String,
    /// Plugin ids explicitly selected for this kit.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// Optional target runtime for the kit.
    pub target: Option<String>,
    /// Capability selectors that pin specific providers.
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
            toml::from_str(&content).map_err(|e| PlatformError::InvalidMetadata(e.to_string()))
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
    /// The loaded kit manifest.
    pub manifest: KitManifest,
    /// Manifests of all plugins available to satisfy the kit's composition.
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

/// A registry of loaded kits.
#[derive(Default)]
pub struct KitRegistry {
    kits: Vec<Kit>,
}

impl KitRegistry {
    /// Creates an empty kit registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a kit.
    pub fn register(&mut self, kit: Kit) {
        self.kits.push(kit);
    }

    /// Returns the kit with the given name, if any.
    pub fn resolve(&self, name: &str) -> Option<&Kit> {
        self.kits.iter().find(|k| k.manifest.name == name)
    }

    /// Returns all registered kits.
    pub fn all(&self) -> &[Kit] {
        &self.kits
    }

    /// Removes the kit with the given name.
    pub fn remove(&mut self, name: &str) -> Result<(), PlatformError> {
        let len_before = self.kits.len();
        self.kits.retain(|k| k.manifest.name != name);
        if self.kits.len() == len_before {
            Err(PlatformError::InvalidMetadata(format!(
                "kit not found: {name}"
            )))
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

        assert!(reg.remove("core").is_ok());
        assert!(matches!(
            reg.remove("core"),
            Err(PlatformError::InvalidMetadata(_))
        ));
    }

    #[test]
    fn kit_manifest_loads_from_files() {
        let dir = std::env::temp_dir();

        let toml = dir.join("handl_kit_manifest.toml");
        std::fs::write(
            &toml,
            r#"id = "core"
name = "core"
plugins = ["fmt"]
"#,
        )
        .unwrap();
        let m = KitManifest::load(&toml).unwrap();
        assert_eq!(m.id, "core");

        let json = dir.join("handl_kit_manifest.json");
        std::fs::write(&json, r#"{"id":"core","name":"core","plugins":["fmt"]}"#).unwrap();
        let m2 = KitManifest::load(&json).unwrap();
        assert_eq!(m2.name, "core");

        let none = dir.join("handl_kit_manifest_noext");
        std::fs::write(&none, "id = \"x\"\nname = \"x\"").unwrap();
        let m3 = KitManifest::load(&none).unwrap();
        assert_eq!(m3.id, "x");

        std::fs::remove_file(&toml).ok();
        std::fs::remove_file(&json).ok();
        std::fs::remove_file(&none).ok();
    }

    #[test]
    fn kit_resolve_uses_available_plugins() {
        let kit = Kit {
            manifest: kit_manifest(),
            available_plugins: vec![PluginManifest {
                id: "fmt".into(),
                version: "0.1.0".into(),
                provides: vec!["format".into()],
                requires: vec![],
                optional: vec![],
                extends: vec![],
                conflicts: vec![],
                replaces: vec![],
                targets: vec![],
                exclusive: vec![],
                metadata: serde_json::Value::Null,
                ast_kinds: Vec::new(),
            }],
        };
        let resolved = kit.resolve().unwrap();
        assert!(resolved.selected_plugins.contains(&"fmt".into()));
    }

    #[test]
    fn kit_manifest_load_failure() {
        let path = std::env::temp_dir().join("handl_kit_does_not_exist.toml");
        let err = KitManifest::load(&path).unwrap_err();
        assert!(matches!(err, PlatformError::InvalidMetadata(_)));
    }
}
