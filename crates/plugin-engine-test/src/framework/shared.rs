use handl_platform::{EvalContext, EvalError};
use serde_json::Value;

use crate::framework::error::TestError;
use crate::framework::function_value::FunctionValue;

pub(crate) fn require_len(args: &[Value], expected: usize) -> Result<(), EvalError> {
    if args.len() != expected {
        return Err(TestError::ArgumentCount {
            expected,
            got: args.len(),
        }
        .into());
    }
    Ok(())
}

pub(crate) fn require_number(value: &Value) -> Result<f64, EvalError> {
    value.as_f64().ok_or_else(|| {
        TestError::TypeMismatch {
            expected: "number",
            got: type_name(value),
        }
        .into()
    })
}

pub(crate) fn require_integer(value: &Value) -> Result<i64, EvalError> {
    let n = require_number(value)?;
    if n.fract() != 0.0 {
        return Err(TestError::TypeMismatch {
            expected: "integer",
            got: type_name(value),
        }
        .into());
    }
    Ok(n as i64)
}

pub(crate) fn require_bool(value: &Value) -> Result<bool, EvalError> {
    value.as_bool().ok_or_else(|| {
        TestError::TypeMismatch {
            expected: "boolean",
            got: type_name(value),
        }
        .into()
    })
}

pub(crate) fn require_string(value: &Value) -> Result<&str, EvalError> {
    value.as_str().ok_or_else(|| {
        TestError::TypeMismatch {
            expected: "string",
            got: type_name(value),
        }
        .into()
    })
}

pub(crate) fn require_array(value: &Value) -> Result<&[Value], EvalError> {
    value.as_array().map(Vec::as_slice).ok_or_else(|| {
        TestError::TypeMismatch {
            expected: "array",
            got: type_name(value),
        }
        .into()
    })
}

pub(crate) fn require_function(value: &Value) -> Result<FunctionValue, EvalError> {
    FunctionValue::from_json(value).ok_or_else(|| {
        TestError::TypeMismatch {
            expected: "function",
            got: type_name(value),
        }
        .into()
    })
}

pub(crate) fn type_name(value: &Value) -> String {
    if value.is_null() {
        "null".to_string()
    } else if value.is_boolean() {
        "boolean".to_string()
    } else if value.is_number() {
        "number".to_string()
    } else if value.is_string() {
        "string".to_string()
    } else if value.is_array() {
        "array".to_string()
    } else if value.is_object() {
        "object".to_string()
    } else {
        "unknown".to_string()
    }
}

pub(crate) fn call_function(
    ctx: &mut EvalContext,
    func: &FunctionValue,
    args: &[Value],
) -> Result<Value, EvalError> {
    if args.len() != func.params.len() {
        return Err(TestError::ArgumentCount {
            expected: func.params.len(),
            got: args.len(),
        }
        .into());
    }

    let vars: Vec<(String, Value)> = func
        .params
        .iter()
        .zip(args.iter())
        .map(|((name, _), value)| (name.clone(), value.clone()))
        .collect();

    ctx.eval_expr(&func.body, &vars)
}

pub(crate) fn b(v: bool) -> Value {
    Value::Bool(v)
}

pub(crate) fn str(v: impl Into<String>) -> Value {
    Value::String(v.into())
}

pub(crate) fn obj(values: &[(&str, serde_json::Value)]) -> Value {
    let mut map = serde_json::Map::new();
    for (k, v) in values {
        map.insert((*k).to_string(), v.clone());
    }
    Value::Object(map)
}
