use super::module::{Dedent, DedentModule};
use handl_platform::Function;
use handl_platform::Module;
use handl_platform::{EvalContext, EvalError};

fn call_dedent(args: &[serde_json::Value]) -> Result<serde_json::Value, EvalError> {
    let mut ctx = EvalContext::new();
    Dedent.call(args, &mut ctx)
}

fn s(v: &str) -> serde_json::Value {
    serde_json::Value::from(v.to_string())
}

#[test]
fn dedent_module_name() {
    assert_eq!(DedentModule.name(), "dedent");
}

#[test]
fn dedent_module_function_lookup() {
    let module = DedentModule;
    assert!(module.function("dedent").is_some());
    assert!(module.function("nonexistent").is_none());
}

#[test]
fn dedent_call_with_string() {
    let result = call_dedent(&[s("  hello\n  world")]).unwrap();
    assert_eq!(result.as_str().unwrap(), "hello\nworld");
}

#[test]
fn dedent_call_with_zero_args() {
    let result = call_dedent(&[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn dedent_call_with_non_string_arg() {
    let result = call_dedent(&[serde_json::Value::from(42.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(msg)) if msg.contains("string")));
}

#[test]
fn dedent_call_with_extra_args() {
    // Extra args are ignored — first arg is used
    let result = call_dedent(&[s("  hi"), s("extra")]).unwrap();
    assert_eq!(result.as_str().unwrap(), "hi");
}

#[test]
fn dedent_call_with_bool_arg() {
    let result = call_dedent(&[serde_json::Value::from(true)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(msg)) if msg.contains("string")));
}

#[test]
fn dedent_call_with_null_arg() {
    let result = call_dedent(&[serde_json::Value::from(())]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(msg)) if msg.contains("string")));
}

#[test]
fn dedent_call_empty_string() {
    let result = call_dedent(&[s("")]).unwrap();
    assert_eq!(result.as_str().unwrap(), "");
}
