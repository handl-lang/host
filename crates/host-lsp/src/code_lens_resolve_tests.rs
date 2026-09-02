//! Tests for `crate::code_lens_resolve`.

use serde_json::json;

use crate::code_lens_resolve::resolve_code_lens;

/// A simple arrow file that refinement inference can handle.
const ARROW_SOURCE: &str = "id >>> id";

/// A non-arrow source (refinement inference returns `None`).
const NON_ARROW_SOURCE: &str = "x = 42";

#[test]
fn resolve_code_lens_adds_command_with_contract() {
    let item = json!({
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 1 },
        },
        "data": { "source": ARROW_SOURCE },
    });
    let resolved = resolve_code_lens(item);
    let command = resolved
        .get("command")
        .expect("command should be filled in by resolve");
    let title = command
        .get("title")
        .and_then(|t| t.as_str())
        .expect("command should have a title");
    assert!(
        title.contains("guarantee"),
        "command title should show the inferred contract: {}",
        title
    );
}

#[test]
fn resolve_code_lens_with_no_data_returns_item_unchanged() {
    let item = json!({
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 1 },
        },
    });
    let resolved = resolve_code_lens(item.clone());
    assert_eq!(resolved, item);
    assert!(resolved.get("command").is_none());
}

#[test]
fn resolve_code_lens_empty_returns_default() {
    let item = json!({});
    let resolved = resolve_code_lens(item);
    assert!(resolved.get("command").is_none());
}

#[test]
fn resolve_code_lens_non_arrow_source_shows_placeholder() {
    let item = json!({
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 1 },
        },
        "data": { "source": NON_ARROW_SOURCE },
    });
    let resolved = resolve_code_lens(item);
    let command = resolved
        .get("command")
        .expect("command should still be set for non-arrow sources");
    let title = command
        .get("title")
        .and_then(|t| t.as_str())
        .expect("command should have a title");
    assert!(
        !title.is_empty(),
        "non-arrow source should get a placeholder title, got: {}",
        title
    );
}
