//! Generic serializer abstraction for the handl runtime.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::SerializeError;

/// A serializer that can turn `serde_json::Value` into a string and back.
pub trait Serializer: Send + Sync {
    /// Serialize a value into a string.
    fn serialize(&self, value: &Value) -> Result<String, SerializeError>;

    /// Deserialize a string into a value.
    fn deserialize(&self, input: &str) -> Result<Value, SerializeError>;
}

/// A registry of named serializers.
#[derive(Default)]
pub struct SerializerRegistry {
    serializers: HashMap<String, Arc<dyn Serializer>>,
}

impl SerializerRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            serializers: HashMap::new(),
        }
    }

    /// Registers a serializer under a name.
    pub fn with(mut self, name: impl Into<String>, serializer: impl Serializer + 'static) -> Self {
        self.serializers.insert(name.into(), Arc::new(serializer));
        self
    }

    /// Returns the serializer registered under `name`, if any.
    pub fn get(&self, name: &str) -> Option<&dyn Serializer> {
        self.serializers.get(name).map(|s| s.as_ref())
    }
}
