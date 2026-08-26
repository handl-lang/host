//! Capability identifiers and metadata.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Opaque metadata associated with a capability, represented as JSON.
pub type CapabilityMetadata = serde_json::Value;

/// A unique capability identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Returns the capability identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CapabilityId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for CapabilityId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A runtime capability that can be requested from a plugin.
pub trait Capability: Send + Sync {
    /// Returns the capability's unique identifier.
    fn id(&self) -> CapabilityId;
    /// Returns the capability's metadata.
    fn metadata(&self) -> CapabilityMetadata;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockCap {
        id: CapabilityId,
        meta: CapabilityMetadata,
    }

    impl Capability for MockCap {
        fn id(&self) -> CapabilityId {
            self.id.clone()
        }

        fn metadata(&self) -> CapabilityMetadata {
            self.meta.clone()
        }
    }

    #[test]
    fn capability_id_from_str_and_string() {
        let a: CapabilityId = "compile".into();
        let b: CapabilityId = "compile".to_string().into();
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "compile");
        assert_eq!(format!("{a}"), "compile");
    }

    #[test]
    fn capability_trait_roundtrip() {
        let cap = MockCap {
            id: "fmt".into(),
            meta: serde_json::json!({"command":"fmt"}),
        };
        assert_eq!(cap.id(), "fmt".into());
        assert_eq!(cap.metadata(), serde_json::json!({"command":"fmt"}));
    }

    #[test]
    fn capability_id_from_string() {
        let s = "eval".to_string();
        let id: CapabilityId = s.into();
        assert_eq!(id.as_str(), "eval");
    }
}
