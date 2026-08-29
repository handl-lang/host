use serde_json::json;

use crate::document::DocumentStore;
use crate::handlers::{handle_definition, handle_references, handle_rename};
use crate::protocol::Message;
use crate::symbols::{find_definition, find_references, identifier_at_offset};

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").unwrap() + 4;
    s[header_end..].to_string()
}

#[test]
fn identifier_at_offset_finds_identifier() {
    let source = "x = 42";
    let result = identifier_at_offset(source, 0);
    assert!(result.is_some());
    let (name, span) = result.unwrap();
    assert_eq!(name, "x");
    assert_eq!(span.start, 0);
    assert_eq!(span.end, 1);
}

#[test]
fn identifier_at_offset_on_operator_returns_none() {
    let source = "x = 42";
    let result = identifier_at_offset(source, 2);
    assert!(result.is_none());
}

#[test]
fn find_definition_finds_definition_site() {
    let source = "x = 42; y = x";
    let span = find_definition(source, "x");
    assert!(span.is_some());
    let span = span.unwrap();
    assert_eq!(span.start, 0);
    assert_eq!(span.end, 1);
}

#[test]
fn find_definition_skips_usage_before_definition() {
    let source = "y = x; x = 42";
    let span = find_definition(source, "x");
    assert!(span.is_some());
    let span = span.unwrap();
    // definition is the second x (in `x = 42`), not the usage in `y = x`
    assert_eq!(span.start, 7);
    assert_eq!(span.end, 8);
}

#[test]
fn find_definition_nonexistent_returns_none() {
    let source = "x = 42";
    let span = find_definition(source, "nonexistent");
    assert!(span.is_none());
}

#[test]
fn find_references_includes_definition_and_usage() {
    let source = "x = 42; y = x";
    let spans = find_references(source, "x");
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].start, 0);
    assert_eq!(spans[1].start, 12);
}

#[test]
fn find_references_no_match_returns_empty() {
    let source = "x = 42";
    let spans = find_references(source, "y");
    assert!(spans.is_empty());
}

#[test]
fn handle_definition_writes_location_array() {
    let mut store = DocumentStore::new();
    store.open("file:///test.handl", "x = 42; y = x");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/definition".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.handl" },
            "position": { "line": 0, "character": 12 }
        })),
        id: Some(json!(1)),
    };
    handle_definition(&mut buf, json!(1), &msg, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 1);
    let locations = parsed["result"].as_array().unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0]["uri"], "file:///test.handl");
    assert_eq!(locations[0]["range"]["start"]["line"], 0);
    assert_eq!(locations[0]["range"]["start"]["character"], 0);
}

#[test]
fn handle_definition_on_whitespace_returns_null() {
    let mut store = DocumentStore::new();
    store.open("file:///test.handl", "x = 42");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/definition".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.handl" },
            "position": { "line": 0, "character": 2 }
        })),
        id: Some(json!(2)),
    };
    handle_definition(&mut buf, json!(2), &msg, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 2);
    assert_eq!(parsed["result"], serde_json::Value::Null);
}

#[test]
fn handle_references_writes_location_array() {
    let mut store = DocumentStore::new();
    store.open("file:///test.handl", "x = 42; y = x");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/references".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.handl" },
            "position": { "line": 0, "character": 0 }
        })),
        id: Some(json!(3)),
    };
    handle_references(&mut buf, json!(3), &msg, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 3);
    let locations = parsed["result"].as_array().unwrap();
    assert_eq!(locations.len(), 2);
    assert_eq!(locations[0]["uri"], "file:///test.handl");
    assert_eq!(locations[1]["uri"], "file:///test.handl");
}

#[test]
fn handle_rename_writes_workspace_edit() {
    let mut store = DocumentStore::new();
    store.open("file:///test.handl", "x = 42; y = x");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/rename".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.handl" },
            "position": { "line": 0, "character": 0 },
            "newName": "z"
        })),
        id: Some(json!(4)),
    };
    handle_rename(&mut buf, json!(4), &msg, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 4);
    let changes = parsed["result"]["changes"].as_object().unwrap();
    assert!(changes.contains_key("file:///test.handl"));
    let edits = changes["file:///test.handl"].as_array().unwrap();
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0]["newText"], "z");
    assert_eq!(edits[1]["newText"], "z");
}
