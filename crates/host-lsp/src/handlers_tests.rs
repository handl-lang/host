use serde_json::json;

use handl_host_tooling::{SemanticTokenLegend, canonical_semantic_tokens};

use crate::document::DocumentStore;
use crate::handlers::{
    build_initialize_result, handle_initialize, handle_log, handle_semantic_tokens_full,
    handle_semantic_tokens_range, handle_shutdown, handle_unknown,
};
use crate::protocol::Message;

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").unwrap() + 4;
    s[header_end..].to_string()
}

#[test]
fn initialize_result_has_semantic_tokens_provider() {
    let legend = SemanticTokenLegend::default();
    let result = build_initialize_result(&legend);

    assert_eq!(result["serverInfo"]["name"], "handl-lsp");
    assert_eq!(
        result["capabilities"]["semanticTokensProvider"]["full"],
        true
    );
    assert_eq!(
        result["capabilities"]["semanticTokensProvider"]["range"],
        true
    );
    assert_eq!(
        result["capabilities"]["textDocumentSync"]["openClose"],
        true
    );
    assert_eq!(result["capabilities"]["textDocumentSync"]["change"], 1);
    assert_eq!(
        result["capabilities"]["textDocumentSync"]["save"]["includeText"],
        true
    );
    assert_eq!(result["capabilities"]["hoverProvider"], true);
    assert!(
        result["capabilities"]["completionProvider"]["triggerCharacters"]
            .as_array()
            .is_some()
    );
    assert_eq!(result["capabilities"]["definitionProvider"], true);
    assert_eq!(result["capabilities"]["referencesProvider"], true);
    assert_eq!(
        result["capabilities"]["renameProvider"]["prepareProvider"],
        true
    );

    let token_types = result["capabilities"]["semanticTokensProvider"]["legend"]["tokenTypes"]
        .as_array()
        .unwrap();
    assert!(!token_types.is_empty());
}

#[test]
fn handle_initialize_writes_response() {
    let legend = SemanticTokenLegend::default();
    let mut buf = Vec::new();
    handle_initialize(&mut buf, json!(1), &legend);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"]["serverInfo"]["name"], "handl-lsp");
}

#[test]
fn handle_shutdown_writes_null_result() {
    let mut buf = Vec::new();
    handle_shutdown(&mut buf, json!(5));

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 5);
    assert_eq!(parsed["result"], serde_json::Value::Null);
}

#[test]
fn handle_unknown_writes_method_not_found() {
    let mut buf = Vec::new();
    handle_unknown(&mut buf, json!(3));

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 3);
    assert_eq!(parsed["error"]["code"], -32601);
    assert_eq!(parsed["error"]["message"], "method not found");
}

#[test]
fn handle_log_writes_log_message() {
    let mut buf = Vec::new();
    handle_log(&mut buf, 2, "warning test");

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["method"], "window/logMessage");
    assert_eq!(parsed["params"]["type"], 2);
    assert_eq!(parsed["params"]["message"], "warning test");
}

#[test]
fn semantic_tokens_full_returns_delta_encoded_data() {
    let provider = canonical_semantic_tokens();

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/semanticTokens/full".to_string(),
        params: None,
        id: Some(json!(1)),
    };

    // We can't easily test with a file URI in unit tests, so the source
    // will be empty and we just verify the response shape.
    let store = DocumentStore::new();
    handle_semantic_tokens_full(&mut buf, json!(1), &msg, &provider, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 1);
    assert!(parsed["result"]["data"].is_array());
}

#[test]
fn semantic_tokens_full_with_actual_source_returns_tokens() {
    let provider = canonical_semantic_tokens();

    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "x = 42").unwrap();
    let uri = format!("file://{}", temp.path().display());

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/semanticTokens/full".to_string(),
        params: Some(json!({ "textDocument": { "uri": uri } })),
        id: Some(json!(2)),
    };

    let store = DocumentStore::new();
    handle_semantic_tokens_full(&mut buf, json!(2), &msg, &provider, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 2);
    let data = parsed["result"]["data"].as_array().unwrap();
    assert!(
        !data.is_empty(),
        "should produce tokens for non-empty source"
    );

    // First token should be "x" (identifier/variable, type 6, length 1,
    // declaration modifier bit set because `x = 42` is a binding).
    assert_eq!(data[0], 0); // delta_line
    assert_eq!(data[1], 0); // delta_start
    assert_eq!(data[2], 1); // length
    assert_eq!(data[3], 6); // token_type = variable
    assert_eq!(data[4], 1); // token_modifiers = declaration
}

#[test]
fn semantic_tokens_range_filters_to_requested_line() {
    let provider = canonical_semantic_tokens();

    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "x = 42\ny = 10").unwrap();
    let uri = format!("file://{}", temp.path().display());

    let mut store = DocumentStore::new();
    store.open(&uri, "x = 42\ny = 10");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/semanticTokens/range".to_string(),
        params: Some(json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 1, "character": 10 }
            }
        })),
        id: Some(json!(3)),
    };
    handle_semantic_tokens_range(&mut buf, json!(3), &msg, &provider, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 3);
    let data = parsed["result"]["data"].as_array().unwrap();
    assert!(!data.is_empty(), "should produce tokens for line 1");
    // Re-encoded from 0: first token's delta_line should be 1 (on line 1).
    assert_eq!(data[0], 1); // delta_line
}

#[test]
fn semantic_tokens_range_whole_document_matches_full() {
    let provider = canonical_semantic_tokens();

    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "x = 42").unwrap();
    let uri = format!("file://{}", temp.path().display());

    let store = DocumentStore::new();

    let mut full_buf = Vec::new();
    let full_msg = Message {
        method: "textDocument/semanticTokens/full".to_string(),
        params: Some(json!({ "textDocument": { "uri": uri } })),
        id: Some(json!(1)),
    };
    handle_semantic_tokens_full(&mut full_buf, json!(1), &full_msg, &provider, &store);
    let full_parsed: serde_json::Value = serde_json::from_str(&extract_body(&full_buf)).unwrap();
    let full_data = full_parsed["result"]["data"].as_array().unwrap();

    let mut range_buf = Vec::new();
    let range_msg = Message {
        method: "textDocument/semanticTokens/range".to_string(),
        params: Some(json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 100 }
            }
        })),
        id: Some(json!(2)),
    };
    handle_semantic_tokens_range(&mut range_buf, json!(2), &range_msg, &provider, &store);
    let range_parsed: serde_json::Value = serde_json::from_str(&extract_body(&range_buf)).unwrap();
    let range_data = range_parsed["result"]["data"].as_array().unwrap();

    assert_eq!(full_data, range_data);
}

#[test]
fn semantic_tokens_range_empty_range_returns_empty_data() {
    let provider = canonical_semantic_tokens();

    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "x = 42").unwrap();
    let uri = format!("file://{}", temp.path().display());

    let store = DocumentStore::new();

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/semanticTokens/range".to_string(),
        params: Some(json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 }
            }
        })),
        id: Some(json!(4)),
    };
    handle_semantic_tokens_range(&mut buf, json!(4), &msg, &provider, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 4);
    let data = parsed["result"]["data"].as_array().unwrap();
    assert!(data.is_empty(), "empty range should produce no tokens");
}
