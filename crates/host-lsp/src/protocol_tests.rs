use serde_json::json;

use crate::protocol::{send_error, send_response};

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").unwrap() + 4;
    s[header_end..].to_string()
}

#[test]
fn send_response_writes_content_length_header() {
    let mut buf = Vec::new();
    send_response(&mut buf, json!(1), json!({"ok": true}));

    let s = String::from_utf8(buf.clone()).unwrap();
    assert!(s.starts_with("Content-Length: "));
    assert!(s.contains("\r\n\r\n"));
}

#[test]
fn send_response_writes_valid_jsonrpc() {
    let mut buf = Vec::new();
    send_response(&mut buf, json!(1), json!({"ok": true}));

    let body = extract_body(&buf);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"]["ok"], true);
}

#[test]
fn send_error_writes_error_code_and_message() {
    let mut buf = Vec::new();
    send_error(&mut buf, json!(2), -32601, "method not found");

    let body = extract_body(&buf);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 2);
    assert_eq!(parsed["error"]["code"], -32601);
    assert_eq!(parsed["error"]["message"], "method not found");
}

#[test]
fn send_response_with_null_id() {
    let mut buf = Vec::new();
    send_response(&mut buf, json!(null), json!(null));

    let body = extract_body(&buf);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(parsed["id"], serde_json::Value::Null);
    assert_eq!(parsed["result"], serde_json::Value::Null);
}

#[test]
fn content_length_matches_body_size() {
    let mut buf = Vec::new();
    send_response(&mut buf, json!(42), json!({"data": [1, 2, 3]}));

    let s = String::from_utf8(buf).unwrap();
    let header_end = s.find("\r\n\r\n").unwrap();
    let header = &s[..header_end];
    let body = &s[header_end + 4..];

    let len: usize = header
        .strip_prefix("Content-Length: ")
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(len, body.len());
}
