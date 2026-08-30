use handl_platform::Module;
use handl_platform::EvalContext;
use handl_platform::{EvalError};

use super::socket::SocketModule;
use super::socket_stub::StubSocket;

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

fn call_err(
    module: &dyn Module,
    name: &str,
    args: &[serde_json::Value],
) -> EvalError {
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
    let module = SocketModule::new(StubSocket::default());
    assert_eq!(module.name(), "socket");
}

#[test]
fn connect_and_recv() {
    let socket = StubSocket::with_binding("echo://test", "hello");
    let module = SocketModule::new(socket);
    let handle = call(&module, "connect", &[s("echo://test")])
        .as_f64()
        .unwrap() as i64;
    let result = call(&module, "recv", &[n(handle), n(1024)]);
    assert_eq!(result.as_str(), Some("hello"));
}

#[test]
fn send_and_close() {
    let socket = StubSocket::default();
    let module = SocketModule::new(socket);
    let handle = call(&module, "connect", &[s("echo://test")])
        .as_f64()
        .unwrap() as i64;
    let sent = call(&module, "send", &[n(handle), s("data")])
        .as_f64()
        .unwrap() as usize;
    assert_eq!(sent, 4);
    call(&module, "close", &[n(handle)]);
    let err = call_err(&module, "recv", &[n(handle), n(1024)]);
    assert!(matches!(
        err,
        EvalError::Other(_)
    ));
}

#[test]
fn unknown_function() {
    let module = SocketModule::new(StubSocket::default());
    assert!(module.function("nope").is_none());
}

#[test]
fn argument_count() {
    let module = SocketModule::new(StubSocket::default());
    let err = call_err(&module, "connect", &[]);
    assert_eq!(
        err,
        EvalError::TypeMismatch(format!("expected {} args, got {}", 1, 0))
    );
}
