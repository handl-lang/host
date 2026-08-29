//! # handl-plugin-lib-encoding
//!
//! Encoding abstractions for the handl runtime.
//!
//! Provides the `Encoding` trait and a registry for codecs such as
//! `base64` and `hex`.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;

/// Errors that can occur when encoding or decoding.
#[derive(Debug, Error)]
pub enum EncodingError {
    #[error("encoding error: {0}")]
    Message(String),
}

/// An encoding that can turn values into strings and back again.
pub trait Encoding: Send + Sync {
    /// Encode `value` according to this encoding.
    fn encode(&self, value: &Value) -> Result<Value, EncodingError>;

    /// Decode `value` from this encoding.
    fn decode(&self, value: &Value) -> Result<Value, EncodingError>;
}

/// A registry of named encodings.
#[derive(Default)]
pub struct EncodingRegistry {
    encodings: HashMap<String, Arc<dyn Encoding>>,
}

impl EncodingRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            encodings: HashMap::new(),
        }
    }

    /// Registers an encoding under a name.
    pub fn with(mut self, name: impl Into<String>, encoding: impl Encoding + 'static) -> Self {
        self.encodings.insert(name.into(), Arc::new(encoding));
        self
    }

    /// Returns the encoding registered under `name`, if any.
    pub fn get(&self, name: &str) -> Option<&dyn Encoding> {
        self.encodings.get(name).map(|e| e.as_ref())
    }
}
