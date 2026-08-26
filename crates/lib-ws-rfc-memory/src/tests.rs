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

fn call_err(name: &str, args: &[NonFunctionValue], ctx: &mut CanonicalContext) {
    WsRfcModule
        .function(name)
        .unwrap()
        .call(args, ctx)
        .expect_err(name);
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

#[test]
fn frame_encode_rejects_invalid_opcode() {
    let mut ctx = ctx();
    let frame = NonFunctionValue::new(serde_json::json!({
        "opcode": "not-a-real-opcode",
        "payload": "aGVsbG8=",
    }));
    call_err("frame.encode", &[frame], &mut ctx);
}

#[test]
fn frame_encode_rejects_invalid_base64_payload() {
    let mut ctx = ctx();
    let frame = NonFunctionValue::new(serde_json::json!({
        "opcode": "text",
        "payload": "!!!",
    }));
    call_err("frame.encode", &[frame], &mut ctx);
}

#[test]
fn frame_encode_rejects_invalid_mask() {
    let mut ctx = ctx();
    let frame = NonFunctionValue::new(serde_json::json!({
        "opcode": "text",
        "payload": "aGVsbG8=",
        "mask": "aGVsbG8=",
    }));
    call_err("frame.encode", &[frame], &mut ctx);
}

#[test]
fn frame_encode_rejects_mask_non_string() {
    let mut ctx = ctx();
    let frame = NonFunctionValue::new(serde_json::json!({
        "opcode": "text",
        "payload": "aGVsbG8=",
        "mask": 123,
    }));
    call_err("frame.encode", &[frame], &mut ctx);
}

#[test]
fn frame_encode_rejects_fin_non_bool() {
    let mut ctx = ctx();
    let frame = NonFunctionValue::new(serde_json::json!({
        "opcode": "text",
        "payload": "aGVsbG8=",
        "fin": "yes",
    }));
    call_err("frame.encode", &[frame], &mut ctx);
}



#[test]
fn frame_decode_rejects_non_base64() {
    let mut ctx = ctx();
    call_err("frame.decode", &[NonFunctionValue::new(serde_json::json!("not-base64!!!"))], &mut ctx);
}

#[test]
fn frame_encode_rejects_non_object() {
    let mut ctx = ctx();
    call_err("frame.encode", &[NonFunctionValue::new(serde_json::json!("not an object"))], &mut ctx);
}

#[test]
fn message_from_frame_rejects_non_object() {
    let mut ctx = ctx();
    call_err("message.from_frame", &[NonFunctionValue::new(serde_json::json!("not an object"))], &mut ctx);
}

#[test]
fn recv_rejects_bad_client_id_type() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({"client_id": "not-a-number"}));
    call_err("recv", &[bad], &mut ctx);
}

#[test]
fn recv_rejects_missing_client_id() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({"type": "websocket_client"}));
    call_err("recv", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_invalid_object() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({"unknown": "value"}));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_bad_binary_base64() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({"binary": "not-base64"}));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_bad_close_code() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({
        "close": {
            "code": 70000,
            "reason": "nope",
        },
    }));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_close_code_non_number() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({
        "close": {
            "code": "1000",
            "reason": "nope",
        },
    }));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_close_code_negative() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({
        "close": {
            "code": -1,
            "reason": "nope",
        },
    }));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_close_code_fractional() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({
        "close": {
            "code": 1000.5,
            "reason": "nope",
        },
    }));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_close_missing_code() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({
        "close": {
            "reason": "nope",
        },
    }));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_close_non_object() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({"close": "bad"}));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_text_non_string() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({"text": 123}));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn message_to_frame_rejects_binary_non_string() {
    let mut ctx = ctx();
    let bad = NonFunctionValue::new(serde_json::json!({"binary": 123}));
    call_err("message.to_frame", &[bad], &mut ctx);
}

#[test]
fn module_returns_none_for_unknown_function() {
    assert!(WsRfcModule.function("unknown").is_none());
}

#[test]
fn message_to_frame_close_null() {
    let mut ctx = ctx();
    let msg = NonFunctionValue::new(serde_json::json!({"close": null}));
    let encoded = call("message.to_frame", &[msg], &mut ctx);
    assert!(encoded.inner().is_string());
}

#[test]
fn message_to_frame_rejects_empty_object() {
    let mut ctx = ctx();
    call_err("message.to_frame", &[NonFunctionValue::new(serde_json::json!({}))], &mut ctx);
}

#[test]
fn frame_encode_rejects_missing_opcode() {
    let mut ctx = ctx();
    let frame = NonFunctionValue::new(serde_json::json!({"payload": "aGVsbG8="}));
    call_err("frame.encode", &[frame], &mut ctx);
}

#[test]
fn client_rejects_arguments() {
    let mut ctx = ctx();
    call_err("client", &[NonFunctionValue::new(serde_json::json!(null))], &mut ctx);
}

#[test]
fn connect_requires_arguments() {
    let mut ctx = ctx();
    let c = call("client", &[], &mut ctx);
    call_err("connect", &[c], &mut ctx);
}

#[test]
fn send_requires_arguments() {
    let mut ctx = ctx();
    let c = call("client", &[], &mut ctx);
    call_err("send", &[c], &mut ctx);
}

#[test]
fn recv_requires_arguments() {
    let mut ctx = ctx();
    call_err("recv", &[], &mut ctx);
}

#[test]
fn recv_rejects_string_client_id() {
    let mut ctx = ctx();
    call_err("recv", &[NonFunctionValue::new(serde_json::json!("not-an-id"))], &mut ctx);
}

#[test]
fn connect_rejects_non_string_url() {
    let mut ctx = ctx();
    let c = call("client", &[], &mut ctx);
    call_err("connect", &[c, NonFunctionValue::new(serde_json::json!(123))], &mut ctx);
}

#[test]
fn close_rejects_string_client_id() {
    let mut ctx = ctx();
    call_err("close", &[NonFunctionValue::new(serde_json::json!("not-an-id"))], &mut ctx);
}

#[test]
fn send_to_unknown_client_fails() {
    let mut ctx = ctx();
    call_err(
        "send",
        &[
            NonFunctionValue::new(serde_json::json!({"client_id": 99999})),
            NonFunctionValue::new(serde_json::json!("hello")),
        ],
        &mut ctx,
    );
}

#[test]
fn recv_from_unknown_client_fails() {
    let mut ctx = ctx();
    call_err("recv", &[NonFunctionValue::new(serde_json::json!({"client_id": 99999}))], &mut ctx);
}

#[test]
fn listen_unknown_server_fails() {
    let mut ctx = ctx();
    let handler = NonFunctionValue::new(serde_json::json!({
        "params": [{"name": "conn", "schema": null}, {"name": "msg", "schema": null}],
        "body": "msg",
    }));
    call_err(
        "listen",
        &[
            NonFunctionValue::new(serde_json::json!({"server_id": 99999})),
            NonFunctionValue::new(serde_json::json!("ws://127.0.0.1:0")),
            handler,
        ],
        &mut ctx,
    );
}

#[test]
fn close_requires_arguments() {
    let mut ctx = ctx();
    call_err("close", &[], &mut ctx);
}

#[test]
fn server_rejects_arguments() {
    let mut ctx = ctx();
    call_err("server", &[NonFunctionValue::new(serde_json::json!(null))], &mut ctx);
}

#[test]
fn listen_requires_arguments() {
    let mut ctx = ctx();
    let s = call("server", &[], &mut ctx);
    call_err("listen", &[s], &mut ctx);
}

#[test]
fn stop_requires_arguments() {
    let mut ctx = ctx();
    call_err("stop", &[], &mut ctx);
}

#[test]
fn frame_decode_rejects_invalid_frame_bytes() {
    let mut ctx = ctx();
    let encoded = NonFunctionValue::new(serde_json::json!("aGVsbG8="));
    call_err("frame.decode", &[encoded], &mut ctx);
}
