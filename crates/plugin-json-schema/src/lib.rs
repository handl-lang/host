use handl_dialect::{Schema, ValidationError};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_vocabulary::SchemaVocabularyPlugin;
use handl_vm_values_json::NonFunctionValue;
use serde_json::Value;

/// A JSON Schema document that can validate `NonFunctionValue`s.
///
/// TODO: Refactor to reuse the canonical `JsonSchema` from
/// `handl-dialect-canonical` once the plugin extraction is complete.
/// For now this is a minimal copy of the validation logic.
pub struct JsonSchema {
    /// The raw schema document.
    pub raw: Value,
}

impl JsonSchema {
    /// Creates a JSON schema from a `serde_json::Value`.
    pub fn new(raw: Value) -> Self {
        JsonSchema { raw }
    }
}

impl Schema<NonFunctionValue> for JsonSchema {
    fn validate(&self, value: &NonFunctionValue) -> Result<(), ValidationError> {
        let validator = jsonschema::draft202012::new(&self.raw).map_err(|e| ValidationError {
            message: format!("invalid JSON schema: {e}"),
            path: String::new(),
            location: None,
        })?;

        validator
            .validate(value.inner())
            .map_err(|e| ValidationError {
                message: e.to_string(),
                path: e.instance_path().to_string(),
                location: None,
            })
    }
}

/// JSON Schema draft 2020-12 vocabulary plugin.
pub struct JsonSchemaVocabulary {
    name: &'static str,
    capabilities: Vec<CapabilityId>,
}

impl JsonSchemaVocabulary {
    /// Creates the JSON Schema vocabulary plugin.
    pub fn new() -> Self {
        Self {
            name: "json-schema-vocabulary",
            capabilities: vec!["vocabulary.schema.json".into()],
        }
    }
}

impl Default for JsonSchemaVocabulary {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaVocabularyPlugin for JsonSchemaVocabulary {
    fn id(&self) -> &'static str {
        "json-schema-draft-2020-12"
    }

    fn validate(&self, value: &NonFunctionValue, schema: &Value) -> Result<(), ValidationError> {
        let compiled = JsonSchema::new(schema.clone());
        compiled.validate(value)
    }
}

impl Plugin for JsonSchemaVocabulary {
    fn name(&self) -> &str {
        self.name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        if self.capabilities.contains(capability) {
            Some(serde_json::json!({"vocabulary": self.id()}))
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

    fn schema() -> Value {
        serde_json::json!({"type": "number"})
    }

    #[test]
    fn accepts_number_against_number_schema() {
        let v = JsonSchemaVocabulary::new();
        assert!(v
            .validate(&NonFunctionValue::new(42), &schema())
            .is_ok());
    }

    #[test]
    fn rejects_string_against_number_schema() {
        let v = JsonSchemaVocabulary::new();
        assert!(v
            .validate(&NonFunctionValue::new("hi"), &schema())
            .is_err());
    }

    #[test]
    fn plugin_reports_capability() {
        let v = JsonSchemaVocabulary::new();
        assert_eq!(v.id(), "json-schema-draft-2020-12");
        assert!(v.capabilities().iter().any(|c| c.as_str() == "vocabulary.schema.json"));
        assert!(v.metadata(&"vocabulary.schema.json".into()).is_some());
    }
}
