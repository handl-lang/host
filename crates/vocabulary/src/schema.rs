use handl_dialect::ValidationError;
use handl_vm_values_json::NonFunctionValue;
use serde_json::Value;

/// In-process plugin interface for schema vocabularies.
pub trait SchemaVocabularyPlugin: Send + Sync {
    /// The vocabulary identifier.
    fn id(&self) -> &'static str;

    /// Validates `value` against a raw schema document.
    fn validate(&self, value: &NonFunctionValue, schema: &Value) -> Result<(), ValidationError>;
}
