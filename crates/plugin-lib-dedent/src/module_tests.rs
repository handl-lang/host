use super::module::{Dedent, DedentModule};
use handl_plugin_vm::Function;
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::{CanonicalContext, Error, NonFunctionValue};

fn call_dedent(args: &[NonFunctionValue]) -> Result<NonFunctionValue, Error> {
    let mut ctx = CanonicalContext::new();
    Dedent.call(args, &mut ctx)
}

fn s(v: &str) -> NonFunctionValue {
    NonFunctionValue::new(v.to_string())
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
fn dedent_module_lazy_function_returns_none() {
    let module = DedentModule;
    assert!(module.lazy_function("dedent").is_none());
    assert!(module.lazy_function("anything").is_none());
}

#[test]
fn dedent_call_with_string() {
    let result = call_dedent(&[s("  hello\n  world")]).unwrap();
    assert_eq!(result.as_str().unwrap(), "hello\nworld");
}

#[test]
fn dedent_call_with_zero_args() {
    let result = call_dedent(&[]);
    assert!(matches!(result, Err(Error::ArgumentCount { expected: 1, got: 0 })));
}

#[test]
fn dedent_call_with_non_string_arg() {
    let result = call_dedent(&[NonFunctionValue::new(42.0)]);
    assert!(matches!(result, Err(Error::TypeMismatch { expected: "string", .. })));
}

#[test]
fn dedent_call_with_extra_args() {
    // Extra args are ignored — first arg is used
    let result = call_dedent(&[s("  hi"), s("extra")]).unwrap();
    assert_eq!(result.as_str().unwrap(), "hi");
}

#[test]
fn dedent_call_with_bool_arg() {
    let result = call_dedent(&[NonFunctionValue::new(true)]);
    assert!(matches!(result, Err(Error::TypeMismatch { expected: "string", .. })));
}

#[test]
fn dedent_call_with_null_arg() {
    let result = call_dedent(&[NonFunctionValue::new(())]);
    assert!(matches!(result, Err(Error::TypeMismatch { expected: "string", .. })));
}

#[test]
fn dedent_call_empty_string() {
    let result = call_dedent(&[s("")]).unwrap();
    assert_eq!(result.as_str().unwrap(), "");
}
