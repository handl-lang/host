use handl_platform::{EvalContext, EvalError, Module};

use crate::PrimModule;

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

fn call_err(
    module: &dyn Module,
    name: &str,
    args: &[serde_json::Value],
) -> Result<serde_json::Value, EvalError> {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
}

fn n(v: i64) -> serde_json::Value {
    serde_json::Value::from(v)
}

fn s(v: &str) -> serde_json::Value {
    serde_json::Value::from(v.to_string())
}

#[test]
fn module_name() {
    assert_eq!(PrimModule.name(), "prim");
}

#[test]
fn unknown_function() {
    assert!(PrimModule.function("nope").is_none());
}

#[test]
fn add_positive() {
    assert_eq!(
        call(&PrimModule, "add", &[n(2), n(3)]).as_i64(),
        Some(5)
    );
}

#[test]
fn add_negative() {
    assert_eq!(
        call(&PrimModule, "add", &[n(-4), n(7)]).as_i64(),
        Some(3)
    );
}

#[test]
fn add_zero() {
    assert_eq!(
        call(&PrimModule, "add", &[n(0), n(0)]).as_i64(),
        Some(0)
    );
}

#[test]
fn add_missing_args_errors() {
    let result = call_err(&PrimModule, "add", &[n(1)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn add_non_integer_errors() {
    let result = call_err(&PrimModule, "add", &[s("a"), n(1)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn compare_less() {
    assert_eq!(
        call(&PrimModule, "compare", &[n(1), n(2)]).as_i64(),
        Some(-1)
    );
}

#[test]
fn compare_equal() {
    assert_eq!(
        call(&PrimModule, "compare", &[n(2), n(2)]).as_i64(),
        Some(0)
    );
}

#[test]
fn compare_greater() {
    assert_eq!(
        call(&PrimModule, "compare", &[n(3), n(1)]).as_i64(),
        Some(1)
    );
}

#[test]
fn compare_strings() {
    assert_eq!(
        call(&PrimModule, "compare", &[s("a"), s("b")]).as_i64(),
        Some(-1)
    );
}

#[test]
fn compare_missing_args_errors() {
    let result = call_err(&PrimModule, "compare", &[n(1)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn alloc_returns_empty_array() {
    let result = call(&PrimModule, "alloc", &[n(5)]);
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn alloc_zero_returns_empty_array() {
    let result = call(&PrimModule, "alloc", &[n(0)]);
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn alloc_missing_args_errors() {
    let result = call_err(&PrimModule, "alloc", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}
