use serde_json::json;

use crate::completion::completions;
use crate::document::DocumentStore;
use crate::handlers::handle_completion;
use crate::protocol::Message;

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").unwrap() + 4;
    s[header_end..].to_string()
}

#[test]
fn completions_empty_source_returns_only_keywords() {
    let items = completions("");
    assert_eq!(items.len(), 11);
    let match_item = items
        .iter()
        .find(|i| i["label"] == "match")
        .expect("match keyword present");
    assert_eq!(match_item["kind"], 14);
    assert_eq!(match_item["detail"], "keyword");
}

#[test]
fn completions_includes_identifiers() {
    let items = completions("x = 42; y = x");
    let keyword_count = 11;
    assert_eq!(items.len(), keyword_count + 2);

    let x = items.iter().find(|i| i["label"] == "x").expect("x present");
    assert_eq!(x["kind"], 6);
    assert_eq!(x["detail"], "identifier");

    let y = items.iter().find(|i| i["label"] == "y").expect("y present");
    assert_eq!(y["kind"], 6);
    assert_eq!(y["detail"], "identifier");

    let x_count = items.iter().filter(|i| i["label"] == "x").count();
    assert_eq!(x_count, 1, "x should appear once");
}

#[test]
fn completions_lex_error_still_returns_keywords() {
    let items = completions("x = \"unterminated");
    assert_eq!(items.len(), 11);
    assert!(items.iter().any(|i| i["label"] == "match"));
}

#[test]
fn handle_completion_writes_response_with_array() {
    let mut store = DocumentStore::new();
    store.open("file:///test.hnl", "x = 42");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/completion".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.hnl" },
            "position": { "line": 0, "character": 0 }
        })),
        id: Some(json!(1)),
    };

    handle_completion(&mut buf, json!(1), &msg, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 1);
    let items = parsed["result"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i["label"] == "match"));
    assert!(items.iter().any(|i| i["label"] == "x"));
}
