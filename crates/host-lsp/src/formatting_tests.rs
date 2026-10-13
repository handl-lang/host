//! Tests for the LSP's `textDocument/formatting` bridge.
//!
//! The LSP has no formatter of its own — it delegates to the canonical
//! `plugin-ext-fmt` engine — so this only checks the response shape it builds.

use crate::formatting::format_document_json;

#[test]
fn format_document_json_returns_valid_json() {
    let source = "x = 1   \n";
    let value = format_document_json(source);
    let arr = value
        .as_array()
        .expect("format_document_json should return a JSON array");
    assert!(!arr.is_empty(), "expected at least one edit");
    for edit in arr {
        assert!(
            edit.get("range").is_some(),
            "edit missing range: {:?}",
            edit
        );
        assert!(
            edit.get("newText").is_some(),
            "edit missing newText: {:?}",
            edit
        );
        assert!(
            edit["range"].get("start").is_some() && edit["range"].get("end").is_some(),
            "edit range missing start/end: {:?}",
            edit
        );
    }
}

#[test]
fn format_document_json_is_empty_for_already_formatted_source() {
    // The canonical formatter is idempotent, so a second pass produces no edits.
    let once = "x = 1\n";
    assert!(
        format_document_json(once)
            .as_array()
            .expect("array")
            .is_empty(),
        "already-canonical source should yield no edits"
    );
}
