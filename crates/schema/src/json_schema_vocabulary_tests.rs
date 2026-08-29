use handl_vm_values_json::NonFunctionValue;

use crate::{JsonSchemaVocabulary, Schema, SchemaVocabulary};

#[test]
fn json_schema_vocabulary_id() {
    assert_eq!(JsonSchemaVocabulary.id(), "json-schema-draft-2020-12");
}

#[test]
fn json_schema_vocabulary_compiles_string_schema() {
    let raw = serde_json::json!({"type": "string"});
    let schema = JsonSchemaVocabulary.compile(&raw);
    assert!(schema.is_ok());
}

#[test]
fn json_schema_validates_string() {
    let raw = serde_json::json!({"type": "string"});
    let schema = JsonSchemaVocabulary.compile(&raw).unwrap();
    let value = NonFunctionValue::new(serde_json::json!("hello"));
    assert!(schema.validate(&value).is_ok());
}

#[test]
fn json_schema_rejects_number_for_string() {
    let raw = serde_json::json!({"type": "string"});
    let schema = JsonSchemaVocabulary.compile(&raw).unwrap();
    let value = NonFunctionValue::new(serde_json::json!(42));
    assert!(schema.validate(&value).is_err());
}

#[test]
fn json_schema_rejects_invalid_schema_document() {
    let raw = serde_json::json!("not a schema");
    let result = JsonSchemaVocabulary.compile(&raw);
    assert!(result.is_err());
}
