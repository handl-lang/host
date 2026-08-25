use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_vocabulary::{DocumentVocabularyPlugin, KeyKind};

/// Canonical document vocabulary plugin.
pub struct CanonicalDocument {
    name: &'static str,
    capabilities: Vec<CapabilityId>,
}

impl CanonicalDocument {
    /// Creates the canonical document vocabulary plugin.
    pub fn new() -> Self {
        Self {
            name: "canonical-document-vocabulary",
            capabilities: vec!["vocabulary.document".into()],
        }
    }
}

impl Default for CanonicalDocument {
    fn default() -> Self {
        Self::new()
    }
}

const SPECIAL_KEYS: &[&str] = &[
    "$", "expr", "arg", "merge", "inheritable", "mutable", "params", "return", "body", "as",
];

impl DocumentVocabularyPlugin for CanonicalDocument {
    fn is_special_key(&self, key: &str) -> bool {
        SPECIAL_KEYS.contains(&key)
    }

    fn classify_key(&self, key: &str) -> KeyKind {
        match key {
            "$" => KeyKind::Field,
            "expr" => KeyKind::Expr,
            "arg" => KeyKind::Arg,
            "merge" => KeyKind::Merge,
            "inheritable" => KeyKind::Inheritable,
            "mutable" => KeyKind::Mutable,
            "params" => KeyKind::Params,
            "return" => KeyKind::Return,
            "body" => KeyKind::Body,
            "as" => KeyKind::Alias,
            _ => KeyKind::Ordinary,
        }
    }

    fn is_reserved(&self, key: &str) -> bool {
        self.is_special_key(key)
    }
}

impl Plugin for CanonicalDocument {
    fn name(&self) -> &str {
        self.name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        if self.capabilities.contains(capability) {
            Some(serde_json::json!({"version": "1"}))
        } else {
            None
        }
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_special_keys() {
        let v = CanonicalDocument::new();
        assert!(v.is_special_key("expr"));
        assert!(v.is_special_key("arg"));
        assert!(v.is_special_key("return"));
    }

    #[test]
    fn classifies_keys() {
        let v = CanonicalDocument::new();
        assert_eq!(v.classify_key("expr"), KeyKind::Expr);
        assert_eq!(v.classify_key("arg"), KeyKind::Arg);
        assert_eq!(v.classify_key("return"), KeyKind::Return);
        assert_eq!(v.classify_key("foo"), KeyKind::Ordinary);
    }

    #[test]
    fn ordinary_key_is_not_special_or_reserved() {
        let v = CanonicalDocument::new();
        assert!(!v.is_special_key("foo"));
        assert!(!v.is_reserved("foo"));
    }

    #[test]
    fn plugin_reports_capability() {
        let v = CanonicalDocument::new();
        assert_eq!(v.name(), "canonical-document-vocabulary");
        assert!(v.capabilities().iter().any(|c| c.as_str() == "vocabulary.document"));
        assert!(v.metadata(&"vocabulary.document".into()).is_some());
    }
}
