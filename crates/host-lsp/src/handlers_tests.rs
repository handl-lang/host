use serde_json::json;

use handl_host_tooling::{SemanticTokenLegend, canonical_semantic_tokens};

use crate::handlers::{
    build_initialize_result, handle_initialize, handle_semantic_tokens_full, handle_shutdown,
    handle_unknown,
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
        false
    );
    assert_eq!(result["capabilities"]["textDocumentSync"], 1);

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

    let parsed: serde_json::Value =
        serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"]["serverInfo"]["name"], "handl-lsp");
}

#[test]
fn handle_shutdown_writes_null_result() {
    let mut buf = Vec::new();
    handle_shutdown(&mut buf, json!(5));

    let parsed: serde_json::Value =
        serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 5);
    assert_eq!(parsed["result"], serde_json::Value::Null);
}

#[test]
fn handle_unknown_writes_method_not_found() {
    let mut buf = Vec::new();
    handle_unknown(&mut buf, json!(3));

    let parsed: serde_json::Value =
        serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 3);
    assert_eq!(parsed["error"]["code"], -32601);
    assert_eq!(parsed["error"]["message"], "method not found");
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
    handle_semantic_tokens_full(&mut buf, json!(1), &msg, &provider);

    let parsed: serde_json::Value =
        serde_json::from_str(&extract_body(&buf)).unwrap();

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

    handle_semantic_tokens_full(&mut buf, json!(2), &msg, &provider);

    let parsed: serde_json::Value =
        serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["id"], 2);
    let data = parsed["result"]["data"].as_array().unwrap();
    assert!(!data.is_empty(), "should produce tokens for non-empty source");

    // First token should be "x" (identifier/variable, type 6, length 1)
    assert_eq!(data[0], 0); // delta_line
    assert_eq!(data[1], 0); // delta_start
    assert_eq!(data[2], 1); // length
    assert_eq!(data[3], 6); // token_type = variable
    assert_eq!(data[4], 0); // token_modifiers = none
}
