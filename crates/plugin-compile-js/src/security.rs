//! Foreign-call descriptor validation and inlining safety.
//!
//! Part of ADR-0114 Phase 7: security hardening for the JavaScript compiler.

use handl_plugin_compile::CompileError;
use serde_json::Value;

/// A validated foreign-call descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignDescriptor {
    /// Host module or global namespace.
    pub module: String,
    /// Function name inside the module.
    pub name: String,
    /// Argument type names.
    pub arg_types: Vec<String>,
    /// Return type name.
    pub return_type: String,
}

impl ForeignDescriptor {
    /// Serialize the descriptor back to a JSON object.
    pub fn to_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("module".to_string(), Value::String(self.module.clone()));
        m.insert("name".to_string(), Value::String(self.name.clone()));
        m.insert(
            "arg_types".to_string(),
            Value::Array(
                self.arg_types
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
        );
        m.insert(
            "return_type".to_string(),
            Value::String(self.return_type.clone()),
        );
        Value::Object(m)
    }
}

/// Validate a raw JSON value as a foreign-call descriptor.
pub fn validate_foreign_descriptor(desc: &Value) -> Result<ForeignDescriptor, CompileError> {
    let obj = desc.as_object().ok_or_else(|| {
        CompileError::InvalidInput("foreign descriptor must be a JSON object".to_string())
    })?;
    let module = get_str(obj, "module")?;
    let name = get_str(obj, "name")?;
    let arg_types = get_str_array(obj, "arg_types")?;
    let return_type = get_str(obj, "return_type")?;
    let fd = ForeignDescriptor {
        module,
        name,
        arg_types,
        return_type,
    };
    if descriptor_contains_dangerous(&fd) {
        return Err(CompileError::InvalidInput(
            "foreign descriptor contains a dangerous value".to_string(),
        ));
    }
    Ok(fd)
}

/// Returns `true` if the descriptor is safe to inline as a direct host call.
pub fn is_safe_for_inlining(fd: &ForeignDescriptor) -> bool {
    if !is_static_identifier(&fd.module) || !is_static_identifier(&fd.name) {
        return false;
    }
    if fd.module.is_empty() || fd.name.is_empty() {
        return false;
    }
    for s in std::iter::once(&fd.module)
        .chain(std::iter::once(&fd.name))
        .chain(std::iter::once(&fd.return_type))
        .chain(fd.arg_types.iter())
    {
        if blocked_for_inlining(s) {
            return false;
        }
    }
    if fd.name.eq_ignore_ascii_case("setTimeout")
        && fd
            .arg_types
            .iter()
            .any(|t| t.to_lowercase().contains("string"))
    {
        return false;
    }
    true
}

fn get_str(obj: &serde_json::Map<String, Value>, key: &str) -> Result<String, CompileError> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CompileError::InvalidInput(format!(
                "foreign descriptor missing or non-string field `{key}`"
            ))
        })
        .map(|s| s.to_string())
}

fn get_str_array(
    obj: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Vec<String>, CompileError> {
    let arr = obj.get(key).and_then(Value::as_array).ok_or_else(|| {
        CompileError::InvalidInput(format!(
            "foreign descriptor missing or non-array field `{key}`"
        ))
    })?;
    arr.iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| {
                    CompileError::InvalidInput(format!(
                        "foreign descriptor `{key}` contains a non-string value"
                    ))
                })
                .map(|s| s.to_string())
        })
        .collect()
}

fn descriptor_contains_dangerous(fd: &ForeignDescriptor) -> bool {
    let mut haystacks: Vec<&str> = vec![&fd.module, &fd.name, &fd.return_type];
    haystacks.extend(fd.arg_types.iter().map(|s| s.as_str()));
    haystacks.iter().any(|s| dangerous_string(s))
}

fn dangerous_string(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("<script")
        || lower.contains("eval")
        || lower.contains("function")
        || lower.contains("setimmediate")
        || lower.contains("settimeout")
        || lower.contains("document.write")
}

fn is_static_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

fn blocked_for_inlining(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("eval")
        || lower.contains("function")
        || lower.contains("setimmediate")
        || lower.contains("settimeout")
        || lower.contains("document.write")
        || lower.contains("<script")
}
