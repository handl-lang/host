use crate::diagnostics::{collect_diagnostics, offset_to_position, parse_diagnostics};
use crate::handlers::handle_publish_diagnostics;
use crate::refinement::RefinementEngine;

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
    let engine = RefinementEngine::new();
    let mut buf = Vec::new();
    handle_publish_diagnostics(&mut buf, "file:///foo.hnl", "x = 42", &engine);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["method"], "textDocument/publishDiagnostics");
    assert_eq!(parsed["params"]["uri"], "file:///foo.hnl");
    assert!(parsed["params"]["diagnostics"].is_array());
    assert!(parsed.get("id").is_none() || parsed["id"].is_null());
}

#[test]
fn handle_publish_diagnostics_with_error_includes_diagnostic() {
    let engine = RefinementEngine::new();
    let mut buf = Vec::new();
    handle_publish_diagnostics(&mut buf, "file:///foo.hnl", "x = \"oops", &engine);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();

    let diags = parsed["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["severity"], 1);
}

#[test]
fn handle_publish_diagnostics_clears_when_valid() {
    let engine = RefinementEngine::new();
    let mut buf = Vec::new();
    handle_publish_diagnostics(&mut buf, "file:///foo.hnl", "arr(\"id\")", &engine);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    let diags = parsed["params"]["diagnostics"].as_array().unwrap();
    assert!(diags.is_empty(), "valid source should clear diagnostics");
}

#[test]
fn parse_diagnostics_for_valid_source_returns_empty() {
    // A valid arrow file should produce no parse diagnostics.
    let diags = parse_diagnostics("arr(\"id\")");
    assert!(
        diags.is_empty(),
        "valid arrow source should produce no parse diagnostics, got: {:?}",
        diags
    );
}

#[test]
fn parse_diagnostics_for_unterminated_string_reports_error() {
    // An unterminated string literal fails lexing; parse_diagnostics should
    // surface a single error diagnostic pinned to line 0.
    let diags = parse_diagnostics("arr(\"unterminated)");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].line, 0);
    assert!(!diags[0].message.is_empty());
}

#[test]
fn parse_diagnostics_for_invalid_syntax_reports_error() {
    // `x =` lexes cleanly but is not a valid arrow file, so the arrow parser
    // should reject it and parse_diagnostics should report one error.
    let diags = parse_diagnostics("x =");
    assert_eq!(diags.len(), 1);
    assert!(!diags[0].message.is_empty());
}

#[test]
fn handle_publish_diagnostics_includes_parse_errors() {
    let engine = RefinementEngine::new();
    let mut buf = Vec::new();
    // `x =` is invalid arrow syntax: parse_diagnostics should contribute an
    // Error-severity (1) diagnostic to the published notification.
    handle_publish_diagnostics(&mut buf, "file:///foo.hnl", "x =", &engine);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    let diags = parsed["params"]["diagnostics"].as_array().unwrap();

    let parse_error = diags.iter().find(|d| {
        d.get("source").and_then(|s| s.as_str()) == Some("handl-parse")
            && d.get("severity").and_then(|s| s.as_u64()) == Some(1)
    });
    assert!(
        parse_error.is_some(),
        "expected a handl-parse error diagnostic, got: {:?}",
        diags
    );
}
