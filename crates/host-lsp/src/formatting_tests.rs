//! Tests for `crate::formatting`.

use crate::formatting::{format_document, format_document_json};

#[test]
fn format_document_indents_loop_body() {
    // The loop body lines are flush-left; they should be indented 2 spaces.
    let source = "loop {\nfoo: 1\nbar: 2\n}\n";
    let edits = format_document(source);
    // `foo: 1` and `bar: 2` are a key-value group of equal-length keys, so
    // no colon padding is needed — only the indent changes.
    assert!(
        edits.iter().any(|e| e.new_text == "  foo: 1"),
        "expected an edit indenting `foo: 1`: {:?}",
        edits
    );
    assert!(
        edits.iter().any(|e| e.new_text == "  bar: 2"),
        "expected an edit indenting `bar: 2`: {:?}",
        edits
    );
}

#[test]
fn format_document_aligns_colons_in_object() {
    // `remaining` (9 chars) and `acc` (3 chars) should have their colons
    // aligned by padding `acc` to the same key width.
    let source = "loop {\nremaining: input,\nacc: []\n}\n";
    let edits = format_document(source);
    let acc_edit = edits
        .iter()
        .find(|e| e.new_text.contains("acc"))
        .expect("expected an edit for the `acc` line");
    assert!(
        acc_edit.new_text.starts_with("  acc      :"),
        "expected `acc` padded so its colon aligns with `remaining`: {:?}",
        acc_edit.new_text
    );
    // The `remaining` line should keep its colon at the same column.
    let rem_edit = edits
        .iter()
        .find(|e| e.new_text.contains("remaining"))
        .expect("expected an edit for the `remaining` line");
    assert!(
        rem_edit.new_text.starts_with("  remaining:"),
        "expected `remaining:` unchanged: {:?}",
        rem_edit.new_text
    );
    // Both colons should land at the same character column.
    let acc_colon = acc_edit.new_text.find(':').unwrap();
    let rem_colon = rem_edit.new_text.find(':').unwrap();
    assert_eq!(acc_colon, rem_colon, "colons should be aligned");
}

#[test]
fn format_document_strips_trailing_whitespace() {
    let source = "x = 1   \n";
    let edits = format_document(source);
    assert!(
        edits.iter().any(|e| e.new_text == "x = 1"),
        "expected trailing whitespace removed: {:?}",
        edits
    );
}

#[test]
fn format_document_ensures_final_newline() {
    let source = "x = 1";
    let edits = format_document(source);
    // The line itself is unchanged, so the only edit adds a trailing newline.
    assert!(
        edits.iter().any(|e| e.new_text == "\n"),
        "expected an edit adding a final newline: {:?}",
        edits
    );
}

#[test]
fn format_document_preserves_blank_lines() {
    // A blank line containing only whitespace is stripped to empty but the
    // line itself is preserved (not removed).
    let source = "x = 1\n   \ny = 2\n";
    let edits = format_document(source);
    let blank_edit = edits.iter().find(|e| e.start_line == 1);
    assert!(
        blank_edit.is_some(),
        "expected an edit for the whitespace-only blank line: {:?}",
        edits
    );
    let edit = blank_edit.unwrap();
    assert_eq!(edit.new_text, "", "blank line should become empty");
    // The edit replaces the line content only (single line range).
    assert_eq!(edit.start_line, edit.end_line);
}

#[test]
fn format_document_decrements_indent_on_closing_brace() {
    // The closing brace is over-indented; it should be de-indented to depth 0.
    let source = "loop {\n  foo: 1\n  }\n";
    let edits = format_document(source);
    assert!(
        edits.iter().any(|e| e.start_line == 2 && e.new_text == "}"),
        "expected the closing brace de-indented to column 0: {:?}",
        edits
    );
}

#[test]
fn format_document_empty_source_returns_no_edits() {
    assert!(format_document("").is_empty());
}

#[test]
fn format_document_already_formatted_returns_no_edits() {
    // Already correctly formatted: proper 2-space indent, aligned colons
    // (equal key lengths), single trailing newline.
    let source = "loop {\n  foo: 1\n  bar: 2\n}\n";
    let edits = format_document(source);
    assert!(edits.is_empty(), "expected no edits: {:?}", edits);
}

#[test]
fn format_document_handles_nested_braces() {
    // `x: 1` is inside `inner: {` so it should be indented one level deeper
    // than `inner` (depth 2, i.e. 4 spaces).
    let source = "loop {\n  inner: {\n  x: 1\n  }\n}\n";
    let edits = format_document(source);
    assert!(
        edits.iter().any(|e| e.new_text == "    x: 1"),
        "expected `x: 1` indented to 4 spaces inside the nested brace: {:?}",
        edits
    );
}

#[test]
fn format_document_json_returns_valid_json() {
    let source = "x = 1   \n";
    let value = format_document_json(source);
    let arr = value
        .as_array()
        .expect("format_document_json should return a JSON array");
    assert!(!arr.is_empty(), "expected at least one edit");
    for edit in arr {
        assert!(edit.get("range").is_some(), "edit missing range: {:?}", edit);
        assert!(
            edit.get("newText").is_some(),
            "edit missing newText: {:?}",
            edit
        );
        assert!(
            edit["range"].get("start").is_some()
                && edit["range"].get("end").is_some(),
            "edit range missing start/end: {:?}",
            edit
        );
    }
}
