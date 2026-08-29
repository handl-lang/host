use handl_vm_values_json::NonFunctionValue;
use serde_json::Value;

use crate::schema::Schema;
use crate::schema_vocabulary::SchemaVocabulary;
use crate::validation_error::ValidationError;

/// Canonical JSON Schema draft 2020-12 vocabulary.
///
/// This is the concrete, shipped implementation used by the canonical
/// Handl language. It compiles raw JSON Schema documents into
/// [`JsonSchema`] validators.
pub struct JsonSchemaVocabulary;

impl SchemaVocabulary<NonFunctionValue> for JsonSchemaVocabulary {
    fn id(&self) -> &'static str {
        "json-schema-draft-2020-12"
    }

    fn compile(
        &self,
        raw: &Value,
    ) -> Result<Box<dyn Schema<NonFunctionValue>>, ValidationError> {
        match raw {
            Value::Bool(_) | Value::Object(_) => Ok(Box::new(JsonSchema {
                schema: raw.clone(),
            })),
            _ => Err(ValidationError {
                message: "JSON Schema document must be an object or boolean".to_string(),
                path: String::new(),
                location: None,
            }),
        }
    }
}

/// A compiled JSON Schema validator.
#[derive(Debug, Clone)]
pub struct JsonSchema {
    schema: Value,
}

impl Schema<NonFunctionValue> for JsonSchema {
    fn validate(&self, value: &NonFunctionValue) -> Result<(), ValidationError> {
        validate_value(&self.schema, value.inner(), "")
    }
}

fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{}.{}", parent, child)
    }
}

fn join_index(parent: &str, index: usize) -> String {
    if parent.is_empty() {
        index.to_string()
    } else {
        format!("{}.{}", parent, index)
    }
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) if value.is_i64() || value.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn type_matches(value: &Value, expected: &str) -> bool {
    match expected {
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        _ => value_type(value) == expected,
    }
}

fn validate_value(schema: &Value, value: &Value, path: &str) -> Result<(), ValidationError> {
    match schema {
        Value::Bool(true) => Ok(()),
        Value::Bool(false) => Err(ValidationError {
            message: "schema is false".to_string(),
            path: path.to_string(),
            location: None,
        }),
        Value::Object(obj) => validate_object_schema(obj, value, path),
        _ => Err(ValidationError {
            message: "invalid JSON Schema".to_string(),
            path: path.to_string(),
            location: None,
        }),
    }
}

fn validate_object_schema(
    schema: &serde_json::Map<String, Value>,
    value: &Value,
    path: &str,
) -> Result<(), ValidationError> {
    if let Some(type_val) = schema.get("type") {
        match type_val {
            Value::String(s) => {
                if !type_matches(value, s) {
                    return Err(ValidationError {
                        message: format!("expected type {}", s),
                        path: path.to_string(),
                        location: None,
                    });
                }
            }
            Value::Array(types) => {
                let matches = types.iter().any(|t| {
                    t.as_str()
                        .map(|expected| type_matches(value, expected))
                        .unwrap_or(false)
                });
                if !matches {
                    return Err(ValidationError {
                        message: format!("expected one of {:?}", types),
                        path: path.to_string(),
                        location: None,
                    });
                }
            }
            _ => {
                return Err(ValidationError {
                    message: "invalid 'type' keyword".to_string(),
                    path: path.to_string(),
                    location: None,
                })
            }
        }
    }

    if let Some(const_val) = schema.get("const") {
        if value != const_val {
            return Err(ValidationError {
                message: format!("expected {:?}", const_val),
                path: path.to_string(),
                location: None,
            });
        }
    }

    if let Some(enum_val) = schema.get("enum") {
        if let Value::Array(options) = enum_val {
            if !options.contains(value) {
                return Err(ValidationError {
                    message: format!("expected one of {:?}", options),
                    path: path.to_string(),
                    location: None,
                });
            }
        } else {
            return Err(ValidationError {
                message: "invalid 'enum' keyword".to_string(),
                path: path.to_string(),
                location: None,
            });
        }
    }

    if let Some(required) = schema.get("required") {
        if let Value::Array(keys) = required {
            let obj = value.as_object().ok_or_else(|| ValidationError {
                message: "expected object for 'required'".to_string(),
                path: path.to_string(),
                location: None,
            })?;
            for key in keys {
                if let Value::String(k) = key {
                    if !obj.contains_key(k) {
                        return Err(ValidationError {
                            message: format!("missing required property {}", k),
                            path: join_path(path, k),
                            location: None,
                        });
                    }
                }
            }
        }
    }

    if let Some(properties) = schema.get("properties") {
        if let Value::Object(props) = properties {
            if let Some(obj) = value.as_object() {
                for (key, subschema) in props {
                    if let Some(child) = obj.get(key) {
                        validate_value(subschema, child, &join_path(path, key))?;
                    }
                }
            }
        }
    }

    if let Some(additional) = schema.get("additionalProperties") {
        if let Some(obj) = value.as_object() {
            let properties = schema
                .get("properties")
                .and_then(|p| p.as_object())
                .map(|p| p.keys().collect::<std::collections::HashSet<_>>())
                .unwrap_or_default();
            for key in obj.keys() {
                if properties.contains(key) {
                    continue;
                }
                match additional {
                    Value::Bool(false) => {
                        return Err(ValidationError {
                            message: format!("additional property '{}' not allowed", key),
                            path: join_path(path, key),
                            location: None,
                        });
                    }
                    Value::Bool(true) => {}
                    subschema => {
                        validate_value(subschema, &obj[key], &join_path(path, key))?;
                    }
                }
            }
        }
    }

    if let Some(items) = schema.get("items") {
        if let Some(arr) = value.as_array() {
            for (i, item) in arr.iter().enumerate() {
                validate_value(items, item, &join_index(path, i))?;
            }
        }
    }

    if let Some(min_length) = schema.get("minLength") {
        if let Some(s) = value.as_str() {
            if let Some(n) = min_length.as_u64() {
                if s.chars().count() < n as usize {
                    return Err(ValidationError {
                        message: format!("string shorter than {}", n),
                        path: path.to_string(),
                        location: None,
                    });
                }
            }
        }
    }

    if let Some(max_length) = schema.get("maxLength") {
        if let Some(s) = value.as_str() {
            if let Some(n) = max_length.as_u64() {
                if s.chars().count() > n as usize {
                    return Err(ValidationError {
                        message: format!("string longer than {}", n),
                        path: path.to_string(),
                        location: None,
                    });
                }
            }
        }
    }

    if let Some(minimum) = schema.get("minimum") {
        if let Some(n) = minimum.as_f64() {
            if let Some(v) = value.as_f64() {
                if v < n {
                    return Err(ValidationError {
                        message: format!("value is less than {}", n),
                        path: path.to_string(),
                        location: None,
                    });
                }
            }
        }
    }

    if let Some(maximum) = schema.get("maximum") {
        if let Some(n) = maximum.as_f64() {
            if let Some(v) = value.as_f64() {
                if v > n {
                    return Err(ValidationError {
                        message: format!("value is greater than {}", n),
                        path: path.to_string(),
                        location: None,
                    });
                }
            }
        }
    }

    if let Some(min_items) = schema.get("minItems") {
        if let Some(arr) = value.as_array() {
            if let Some(n) = min_items.as_u64() {
                if arr.len() < n as usize {
                    return Err(ValidationError {
                        message: format!("array has fewer than {} items", n),
                        path: path.to_string(),
                        location: None,
                    });
                }
            }
        }
    }

    if let Some(max_items) = schema.get("maxItems") {
        if let Some(arr) = value.as_array() {
            if let Some(n) = max_items.as_u64() {
                if arr.len() > n as usize {
                    return Err(ValidationError {
                        message: format!("array has more than {} items", n),
                        path: path.to_string(),
                        location: None,
                    });
                }
            }
        }
    }

    Ok(())
}
