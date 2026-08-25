use super::WsRfcModule;
use handl_vm_common::Module;
use handl_vm_values_json::{CanonicalContext, NonFunctionValue};

fn ctx() -> CanonicalContext {
    CanonicalContext::new()
}

fn call(name: &str, args: &[NonFunctionValue], ctx: &mut CanonicalContext) -> NonFunctionValue {
    WsRfcModule
        .function(name)
        .unwrap()
        .call(args, ctx)
        .unwrap()
}

#[test]
fn client_create_connect_close() {
    let mut ctx = ctx();
    let c = call("client", &[], &mut ctx);
    assert_eq!(c.inner().get("type"), Some(&serde_json::json!("websocket_client")));

    let connected = call("connect", &[c.clone(), NonFunctionValue::new(serde_json::json!("ws://test"))], &mut ctx);
    assert_eq!(connected.inner(), &serde_json::json!(null));

    let closed = call("close", &[c], &mut ctx);
    assert_eq!(closed.inner(), &serde_json::json!(null));
}

#[test]
fn client_send_recv_roundtrip() {
    let mut ctx = ctx();
    let c = call("client", &[], &mut ctx);
    call("connect", &[c.clone(), NonFunctionValue::new(serde_json::json!("ws://test"))], &mut ctx);
    call("send", &[c.clone(), NonFunctionValue::new(serde_json::json!("hello"))], &mut ctx);

    let msg = call("recv", &[c.clone()], &mut ctx);
    assert_eq!(msg.inner(), &serde_json::json!("hello"));

    let empty = call("recv", &[c.clone()], &mut ctx);
    assert!(empty.inner().is_null());

    call("close", &[c], &mut ctx);
}

#[test]
fn server_listen_stop() {
    let mut ctx = ctx();
    let s = call("server", &[], &mut ctx);
    assert_eq!(s.inner().get("type"), Some(&serde_json::json!("websocket_server")));

    let handler = NonFunctionValue::new(serde_json::json!({
        "params": [
            {"name": "conn", "schema": null},
            {"name": "msg", "schema": null},
        ],
        "body": "msg",
    }));
    let listened = call("listen", &[
        s.clone(),
        NonFunctionValue::new(serde_json::json!("ws://0.0.0.0:9001")),
        handler,
    ], &mut ctx);
    assert_eq!(listened.inner(), &serde_json::json!(null));

    let stopped = call("stop", &[s], &mut ctx);
    assert_eq!(stopped.inner(), &serde_json::json!(null));
}

#[test]
fn frame_encode_decode_roundtrip() {
    let mut ctx = ctx();
    let frame = NonFunctionValue::new(serde_json::json!({
        "opcode": "text",
        "payload": "aGVsbG8=",
    }));
    let encoded = call("frame.encode", &[frame], &mut ctx);
    assert!(encoded.inner().is_string());

    let decoded = call("frame.decode", &[encoded], &mut ctx);
    assert_eq!(decoded.inner().get("opcode"), Some(&serde_json::json!("text")));
}

#[test]
fn message_from_to_frame_roundtrip() {
    let mut ctx = ctx();
    let frame = NonFunctionValue::new(serde_json::json!({
        "opcode": "text",
        "payload": "aGVsbG8=",
    }));
    let msg = call("message.from_frame", &[frame], &mut ctx);
    assert_eq!(msg.inner().get("text"), Some(&serde_json::json!("hello")));

    let back = call("message.to_frame", &[msg], &mut ctx);
    assert!(back.inner().is_string());
}
