use handl_platform::EvalContext;
use handl_platform::EvalError;
use handl_platform::Module;

use super::sig::SigModule;
use super::sig_stub::StubSig;

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

fn call_err(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> EvalError {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).unwrap_err()
}

fn s(v: &str) -> serde_json::Value {
    serde_json::Value::from(v.to_string())
}

fn n(v: i64) -> serde_json::Value {
    serde_json::Value::from(v)
}

#[test]
fn module_name() {
    let module = SigModule::new(StubSig::default());
    assert_eq!(module.name(), "sig");
}

#[test]
fn list_signals() {
    let module = SigModule::new(StubSig::default());
    let result = call(&module, "list", &[]);
    let arr = result.as_array().unwrap();
    assert!(!arr.is_empty());
}

#[test]
fn send_signal() {
    let module = SigModule::new(StubSig::default());
    call(&module, "send", &[n(42), s("SIGINT")]);
}

#[test]
fn send_unknown_signal() {
    let module = SigModule::new(StubSig::default());
    let err = call_err(&module, "send", &[n(42), s("BOGUS")]);
    assert!(matches!(err, EvalError::Other(_)));
}

#[test]
fn argument_count() {
    let module = SigModule::new(StubSig::default());
    let err = call_err(&module, "send", &[n(42)]);
    assert_eq!(
        err,
        EvalError::TypeMismatch(format!("expected {} args, got {}", 2, 1))
    );
}
