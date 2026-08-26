//! Extension registration and discovery.

use serde_json::Value;

/// A runtime extension identified by a string id.
pub trait Extension: Send + Sync {
    /// Returns the extension's unique identifier.
    fn id(&self) -> &str;
    /// Returns the extension's metadata.
    fn metadata(&self) -> Value;
}

/// A registry of loaded extensions.
#[derive(Default)]
pub struct ExtensionRegistry {
    extensions: Vec<Box<dyn Extension + 'static>>,
}

impl ExtensionRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an extension.
    pub fn register<E: Extension + 'static>(&mut self, extension: E) {
        self.extensions.push(Box::new(extension));
    }

    /// Returns the extension with the given id, if any.
    pub fn get(&self, id: &str) -> Option<&dyn Extension> {
        self.extensions.iter().find(|e| e.id() == id).map(|e| &**e)
    }

    /// Returns all registered extensions.
    pub fn all(&self) -> Vec<&dyn Extension> {
        self.extensions.iter().map(|e| &**e).collect()
    }

    /// Removes the extension with the given id.
    pub fn remove(&mut self, id: &str) -> Result<(), crate::error::PlatformError> {
        let len_before = self.extensions.len();
        self.extensions.retain(|e| e.id() != id);
        if self.extensions.len() == len_before {
            Err(crate::error::PlatformError::InvalidMetadata(format!(
                "extension not found: {id}"
            )))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlatformError;

    #[derive(Debug)]
    struct MockExt {
        id: &'static str,
        meta: Value,
    }

    impl Extension for MockExt {
        fn id(&self) -> &str {
            self.id
        }

        fn metadata(&self) -> Value {
            self.meta.clone()
        }
    }

    #[test]
    fn registry_registers_gets_and_lists() {
        let mut reg = ExtensionRegistry::new();
        reg.register(MockExt {
            id: "foo",
            meta: serde_json::json!({"name":"foo"}),
        });
        reg.register(MockExt {
            id: "bar",
            meta: serde_json::json!({"name":"bar"}),
        });

        assert!(reg.get("foo").is_some());
        assert!(reg.get("baz").is_none());
        assert_eq!(reg.all().len(), 2);
        assert_eq!(
            reg.get("bar").unwrap().metadata(),
            serde_json::json!({"name":"bar"})
        );

        assert!(reg.remove("foo").is_ok());
        assert!(reg.get("foo").is_none());
        assert!(matches!(
            reg.remove("foo"),
            Err(PlatformError::InvalidMetadata(_))
        ));
    }
}
