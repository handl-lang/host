use crate::diagnostics::{collect_diagnostics, offset_to_position};
use crate::handlers::handle_publish_diagnostics;

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").unwrap() + 4;
    s[header_end..].to_string()
}

#[test]
fn valid_source_produces_no_diagnostics() {
    let diags = collect_diagnostics("x = 42");
    assert!(diags.is_empty(), "valid source should have no diagnostics");
}

#[test]
fn unterminated_string_produces_one_error_diagnostic() {
    let diags = collect_diagnostics("x = \"unterminated");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["severity"], 1);
    let start = &diags[0]["range"]["start"];
    let end = &diags[0]["range"]["end"];
    assert!(start["line"].as_u64().is_some());
    assert!(end["line"].as_u64().is_some());
    assert!(start["line"].as_u64().unwrap() <= end["line"].as_u64().unwrap());
}

#[test]
fn parse_error_produces_diagnostic_at_origin() {
    let diags = collect_diagnostics("x =");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["severity"], 1);
    assert_eq!(diags[0]["range"]["start"]["line"], 0);
    assert_eq!(diags[0]["range"]["start"]["character"], 0);
    assert_eq!(diags[0]["range"]["end"]["line"], 0);
    assert_eq!(diags[0]["range"]["end"]["character"], 0);
}

#[test]
fn offset_to_position_at_start() {
    assert_eq!(offset_to_position("hello", 0), (0, 0));
}

#[test]
fn offset_to_position_mid_first_line() {
    assert_eq!(offset_to_position("hello", 2), (0, 2));
}

#[test]
fn offset_to_position_after_newline() {
    assert_eq!(offset_to_position("abc\ndef", 4), (1, 0));
    assert_eq!(offset_to_position("abc\ndef", 6), (1, 2));
}

#[test]
fn offset_to_position_past_end_clamped() {
    assert_eq!(offset_to_position("abc", 100), (0, 3));
}

#[test]
fn handle_publish_diagnostics_writes_notification() {
    let mut buf = Vec::new();
    handle_publish_diagnostics(&mut buf, "file:///foo.hnl", "x = 42");

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["method"], "textDocument/publishDiagnostics");
    assert_eq!(parsed["params"]["uri"], "file:///foo.hnl");
    assert!(parsed["params"]["diagnostics"].is_array());
    assert!(parsed.get("id").is_none() || parsed["id"].is_null());
}

#[test]
fn handle_publish_diagnostics_with_error_includes_diagnostic() {
    let mut buf = Vec::new();
    handle_publish_diagnostics(&mut buf, "file:///foo.hnl", "x = \"oops");

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    let diags = parsed["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["severity"], 1);
}

#[test]
fn handle_publish_diagnostics_clears_when_valid() {
    let mut buf = Vec::new();
    handle_publish_diagnostics(&mut buf, "file:///foo.hnl", "x = 1");

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    let diags = parsed["params"]["diagnostics"].as_array().unwrap();
    assert!(diags.is_empty(), "valid source should clear diagnostics");
}
