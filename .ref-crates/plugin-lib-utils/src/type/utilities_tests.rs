use handl_platform::Module;

use super::TypeModule;
use handl_platform::{EvalContext, EvalError};

fn call(name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = TypeModule.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

fn call_err(name: &str, args: &[serde_json::Value]) -> Result<serde_json::Value, EvalError> {
    let func = TypeModule.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
}

fn n(v: f64) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(v))
}

fn s(v: &str) -> serde_json::Value {
    serde_json::Value::from(v.to_string())
}

fn null() -> serde_json::Value {
    serde_json::Value::from(serde_json::Value::Null)
}

#[test]
fn identity_number() {
    assert_eq!(call("identity", &[n(42.0)]).as_f64(), Some(42.0));
}

#[test]
fn identity_string() {
    assert_eq!(call("identity", &[s("hello")]).as_str(), Some("hello"));
}

#[test]
fn identity_null() {
    assert!(call("identity", &[null()]).is_null());
}

#[test]
fn identity_no_args_errors() {
    let result = call_err("identity", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn default_non_null() {
    assert_eq!(call("default", &[n(42.0), n(0.0)]).as_f64(), Some(42.0));
}

#[test]
fn default_null() {
    assert_eq!(call("default", &[null(), n(0.0)]).as_f64(), Some(0.0));
}

#[test]
fn default_no_args_errors() {
    let result = call_err("default", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn default_one_arg_errors() {
    let result = call_err("default", &[n(1.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn coalesce_first_non_null() {
    assert_eq!(
        call("coalesce", &[n(1.0), n(2.0), n(3.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn coalesce_skips_nulls() {
    assert_eq!(
        call("coalesce", &[null(), null(), n(3.0)]).as_f64(),
        Some(3.0)
    );
}

#[test]
fn coalesce_all_null() {
    assert!(call("coalesce", &[null(), null()]).is_null());
}

#[test]
fn coalesce_no_args_returns_null() {
    assert!(call("coalesce", &[]).is_null());
}
