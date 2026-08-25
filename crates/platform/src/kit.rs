use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Kit {
    pub name: String,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct KitManifest {
    pub metadata: Option<Value>,
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
        self.kits.iter().find(|k| k.name == name)
    }

    pub fn all(&self) -> &[Kit] {
        &self.kits
    }

    pub fn remove(&mut self, name: &str) -> Result<(), crate::error::PlatformError> {
        let len_before = self.kits.len();
        self.kits.retain(|k| k.name != name);
        if self.kits.len() == len_before {
            Err(crate::error::PlatformError::InvalidMetadata(format!("kit not found: {name}")))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kit_registry_register_resolve_and_all() {
        let mut reg = KitRegistry::new();
        reg.register(Kit {
            name: "core".into(),
            plugins: vec!["fmt".into(), "lint".into()],
        });
        reg.register(Kit {
            name: "web".into(),
            plugins: vec!["serve".into()],
        });

        assert_eq!(reg.all().len(), 2);
        assert_eq!(reg.resolve("core").unwrap().plugins.len(), 2);
        assert!(reg.resolve("missing").is_none());
    }

    #[test]
    fn kit_manifest_optional_metadata() {
        let m = KitManifest { metadata: Some(serde_json::json!({"description":"base kit"})) };
        assert!(m.metadata.is_some());
    }
}
