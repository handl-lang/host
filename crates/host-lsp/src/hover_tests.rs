use serde_json::json;

use crate::document::DocumentStore;
use crate::handlers::handle_hover;
use crate::hover::hover_at;
use crate::protocol::Message;

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").unwrap() + 4;
    s[header_end..].to_string()
}

fn contents_value(value: &serde_json::Value) -> &serde_json::Value {
    if value["contents"].is_string() {
        &value["contents"]
    } else {
        &value["contents"]["value"]
    }
}

#[test]
fn hover_identifier_at_start() {
    let value = hover_at("x = 42", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("identifier"), "contents: {contents}");
    assert!(contents.contains("x"), "contents: {contents}");
}

#[test]
fn hover_number_at_value() {
    let value = hover_at("x = 42", 4).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("number"), "contents: {contents}");
    assert!(contents.contains("42"), "contents: {contents}");
}

#[test]
fn hover_keyword_match() {
    let value = hover_at("match", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("keyword"), "contents: {contents}");
}

#[test]
fn hover_whitespace_returns_none() {
    assert!(hover_at("x = 42", 1).is_none());
}

#[test]
fn hover_empty_source_returns_none() {
    assert!(hover_at("", 0).is_none());
}

#[test]
fn handle_hover_writes_response_with_contents() {
    let mut store = DocumentStore::new();
    store.open("file:///test.hnl", "x = 42");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/hover".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.hnl" },
            "position": { "line": 0, "character": 0 }
        })),
        id: Some(json!(1)),
    };

    handle_hover(&mut buf, json!(1), &msg, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 1);
    let contents = parsed["result"]["contents"]["value"].as_str().unwrap();
    assert!(contents.contains("identifier"));
}

#[test]
fn handle_hover_returns_null_in_whitespace() {
    let mut store = DocumentStore::new();
    store.open("file:///test.hnl", "x = 42");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/hover".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.hnl" },
            "position": { "line": 0, "character": 1 }
        })),
        id: Some(json!(2)),
    };

    handle_hover(&mut buf, json!(2), &msg, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 2);
    assert_eq!(parsed["result"], serde_json::Value::Null);
}
