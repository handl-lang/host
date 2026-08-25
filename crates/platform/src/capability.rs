use std::fmt;

pub type CapabilityMetadata = serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapabilityId(String);

impl CapabilityId {
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

pub trait Capability: Send + Sync {
    fn id(&self) -> CapabilityId;
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
}
