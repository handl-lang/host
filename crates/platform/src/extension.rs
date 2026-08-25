use serde_json::Value;

pub trait Extension: Send + Sync {
    fn id(&self) -> &str;
    fn metadata(&self) -> Value;
}

#[derive(Default)]
pub struct ExtensionRegistry {
    extensions: Vec<Box<dyn Extension + 'static>>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<E: Extension + 'static>(&mut self, extension: E) {
        self.extensions.push(Box::new(extension));
    }

    pub fn get(&self, id: &str) -> Option<&dyn Extension> {
        self.extensions.iter().find(|e| e.id() == id).map(|e| &**e)
    }

    pub fn all(&self) -> Vec<&dyn Extension> {
        self.extensions.iter().map(|e| &**e).collect()
    }

    pub fn remove(&mut self, id: &str) -> Result<(), crate::error::PlatformError> {
        let len_before = self.extensions.len();
        self.extensions.retain(|e| e.id() != id);
        if self.extensions.len() == len_before {
            Err(crate::error::PlatformError::InvalidMetadata(format!("extension not found: {id}")))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(reg.get("bar").unwrap().metadata(), serde_json::json!({"name":"bar"}));
    }
}
