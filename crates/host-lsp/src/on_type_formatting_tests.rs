//! Tests for `crate::on_type_formatting`.

use serde_json::Value;

use crate::on_type_formatting::on_type_formatting;

/// Helper: extract the single edit's `newText` from the returned array, or
/// panic if there isn't exactly one.
fn single_new_text(edits: &[Value]) -> String {
    assert_eq!(
        edits.len(),
        1,
        "expected exactly one edit, got {:?}",
        edits
    );
    edits[0]
        .get("newText")
        .and_then(|t| t.as_str())
        .expect("edit has no newText")
        .to_string()
}

/// Helper: assert the returned edits list is empty.
fn assert_no_edits(edits: &[Value]) {
    assert!(edits.is_empty(), "expected no edits, got {:?}", edits);
}

#[test]
fn on_type_formatting_dedents_closing_brace() {
    // The user just typed `}` on line 2. It was auto-indented to column 2,
    // but it should dedent to column 0 to match the `loop {` opener on line 0.
    let source = "loop {\n  foo: 1\n  }\n";
    let edits = on_type_formatting(source, (2, 2), "}");
    let new_text = single_new_text(&edits);
    assert_eq!(new_text, "", "dedented `}}` should have empty indent");
    // The edit should replace the leading whitespace of line 2 (cols 0..2).
    let range = &edits[0]["range"];
    assert_eq!(range["start"]["line"], 2);
    assert_eq!(range["start"]["character"], 0);
    assert_eq!(range["end"]["line"], 2);
    assert_eq!(range["end"]["character"], 2);
}

#[test]
fn on_type_formatting_dedents_closing_paren() {
    // `)` on line 1 is over-indented; it should match the `(` on line 0.
    let source = "foo(\n  bar\n  )\n";
    let edits = on_type_formatting(source, (2, 2), ")");
    let new_text = single_new_text(&edits);
    assert_eq!(new_text, "", "dedented `)` should have empty indent",);
    assert_eq!(edits[0]["range"]["end"]["character"], 2);
}

#[test]
fn on_type_formatting_dedents_closing_bracket() {
    // `]` on line 1 is over-indented; it should match the `[` on line 0.
    let source = "[\n  1, 2, 3\n  ]\n";
    let edits = on_type_formatting(source, (2, 2), "]");
    let new_text = single_new_text(&edits);
    assert_eq!(new_text, "", "dedented `]` should have empty indent",);
    assert_eq!(edits[0]["range"]["end"]["character"], 2);
}

#[test]
fn on_type_formatting_indents_after_open_brace() {
    // User typed Enter after `loop {`. The new (empty) line should gain one
    // indent level (2 spaces).
    let source = "loop {\n";
    let edits = on_type_formatting(source, (0, 6), "\n");
    let new_text = single_new_text(&edits);
    assert_eq!(
        new_text,
        "  ",
        "new line after `{{` should be indented 2 spaces"
    );
    let range = &edits[0]["range"];
    assert_eq!(range["start"]["line"], 1);
    assert_eq!(range["start"]["character"], 0);
    assert_eq!(range["end"]["line"], 1);
    assert_eq!(range["end"]["character"], 0);
}

#[test]
fn on_type_formatting_indents_after_colon() {
    // User typed Enter after a trailing colon. The new line should gain one
    // extra indent level for the value that follows.
    let source = "loop {\n  key:\n";
    let edits = on_type_formatting(source, (1, 6), "\n");
    let new_text = single_new_text(&edits);
    assert_eq!(
        new_text,
        "    ",
        "new line after `:` should be indented one level deeper than the key"
    );
}

#[test]
fn on_type_formatting_no_change_at_top_level() {
    // The `}` is already at column 0, matching the top-level `{` on line 0.
    let source = "{\n}\n";
    let edits = on_type_formatting(source, (1, 0), "}");
    assert_no_edits(&edits);
}

#[test]
fn on_type_formatting_preserves_existing_indent() {
    // The `}` is already correctly indented to match the `loop {` opener, so
    // no edit should be produced.
    let source = "loop {\n  foo: 1\n}\n";
    let edits = on_type_formatting(source, (2, 0), "}");
    assert_no_edits(&edits);
}

#[test]
fn on_type_formatting_empty_source_returns_empty() {
    let edits = on_type_formatting("", (0, 0), "}");
    assert_no_edits(&edits);

    let edits = on_type_formatting("", (0, 0), "\n");
    assert_no_edits(&edits);
}

#[test]
fn on_type_formatting_nested_brackets() {
    // The inner `}` (line 3) was auto-indented to column 4, but it should
    // dedent to column 2 to match the `if x {` opener on line 1.
    let source = "loop {\n  if x {\n    foo\n    }\n  }\n";
    let edits = on_type_formatting(source, (3, 4), "}");
    let new_text = single_new_text(&edits);
    assert_eq!(
        new_text,
        "  ",
        "inner `}}` should dedent to match the `if x {{` line indent"
    );
    let range = &edits[0]["range"];
    assert_eq!(range["start"]["line"], 3);
    assert_eq!(range["start"]["character"], 0);
    assert_eq!(range["end"]["character"], 4);
}
