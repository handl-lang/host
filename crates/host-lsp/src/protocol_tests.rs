use serde_json::json;

use crate::protocol::{send_error, send_response};

#[test]
fn send_response_writes_valid_jsonrpc() {
    let mut buf = Vec::new();
    send_response(&mut buf, json!(1), json!({"ok": true}));

    let line = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"]["ok"], true);
}

#[test]
fn send_error_writes_error_code_and_message() {
    let mut buf = Vec::new();
    send_error(&mut buf, json!(2), -32601, "method not found");

    let line = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 2);
    assert_eq!(parsed["error"]["code"], -32601);
    assert_eq!(parsed["error"]["message"], "method not found");
}

#[test]
fn send_response_with_null_id() {
    let mut buf = Vec::new();
    send_response(&mut buf, json!(null), json!(null));

    let line = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(parsed["id"], serde_json::Value::Null);
    assert_eq!(parsed["result"], serde_json::Value::Null);
}
