//! Tests for `crate::range_formatting`.

use serde_json::Value;

use crate::range_formatting::range_format;

// --- helpers ---------------------------------------------------------------

/// Convert a (line, character) position into a byte offset within `source`.
/// Positions beyond the end of the text clamp to `source.len()`.
fn pos_to_offset(source: &str, line: usize, char: usize) -> usize {
    let mut cur_line = 0;
    let mut cur_char = 0;
    for (i, ch) in source.char_indices() {
        if cur_line == line && cur_char == char {
            return i;
        }
        if ch == '\n' {
            cur_line += 1;
            cur_char = 0;
        } else {
            cur_char += 1;
        }
    }
    source.len()
}

/// Apply a list of `(start_line, start_char, end_line, end_char, new_text)`
/// edits to `source`, returning the resulting text. Edits are applied from
/// the end of the document backward so earlier offsets stay valid.
fn apply_edits(source: &str, mut edits: Vec<(usize, usize, usize, usize, String)>) -> String {
    // Sort by start position descending (line, then char).
    edits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut result = source.to_string();
    for (sl, sc, el, ec, nt) in edits {
        let start = pos_to_offset(&result, sl, sc);
        let end = pos_to_offset(&result, el, ec);
        result.replace_range(start..end, &nt);
    }
    result
}

/// Apply the JSON `TextEdit` objects returned by `range_format` to `source`.
fn apply_range_edits(source: &str, edits: &[Value]) -> String {
    let mut raw: Vec<(usize, usize, usize, usize, String)> = Vec::new();
    for e in edits {
        let range = &e["range"];
        let sl = range["start"]["line"].as_u64().unwrap() as usize;
        let sc = range["start"]["character"].as_u64().unwrap() as usize;
        let el = range["end"]["line"].as_u64().unwrap() as usize;
        let ec = range["end"]["character"].as_u64().unwrap() as usize;
        let nt = e["newText"].as_str().unwrap().to_string();
        raw.push((sl, sc, el, ec, nt));
    }
    apply_edits(source, raw)
}

/// Extract the `newText` of the single edit returned by `range_format`, or
/// `None` when no edits were produced.
fn range_new_text(edits: &[Value]) -> Option<String> {
    edits
        .first()
        .map(|e| e["newText"].as_str().unwrap().to_string())
}

// --- tests -----------------------------------------------------------------

#[test]
fn range_format_formats_selected_lines() {
    // Lines 2-5 (0-based) are flush-left; they should be indented 2 spaces
    // because they sit inside the `loop {` block opened on line 1.
    // 0-based lines:
    //   0: header: 1
    //   1: loop {
    //   2: foo: 1
    //   3: bar: 2
    //   4: baz: 3
    //   5: }
    //   6: trailer: 0
    // Formatting lines 2-5 should indent the key-value lines 2 spaces (depth
    // 1 inside `loop {`) and leave the closing `}` flush-left (depth 0).
    let source = "header: 1\nloop {\nfoo: 1\nbar: 2\nbaz: 3\n}\ntrailer: 0\n";
    let edits = range_format(source, 2, 5);
    assert_eq!(
        edits.len(),
        1,
        "expected exactly one range edit: {:?}",
        edits
    );
    let new_text = range_new_text(&edits).unwrap();
    let lines: Vec<&str> = new_text.trim_end_matches('\n').split('\n').collect();
    assert_eq!(lines.len(), 4, "expected 4 formatted lines: {:?}", new_text);
    // The three key-value lines should be indented 2 spaces.
    for line in &lines[..3] {
        assert!(
            line.starts_with("  "),
            "expected key-value lines to be indented 2 spaces: {:?}",
            line
        );
    }
    // The closing brace should be flush-left.
    assert_eq!(
        lines[3], "}",
        "expected closing brace at indent 0: {:?}",
        lines[3]
    );
    // Lines outside the range are untouched.
    let result = apply_range_edits(source, &edits);
    assert!(result.starts_with("header: 1\nloop {\n"));
    assert!(result.ends_with("\ntrailer: 0\n"));
}

#[test]
fn range_format_respects_bracket_depth() {
    // The selected lines are two levels deep: inside `outer {` then `inner {`.
    let source = "outer {\ninner {\nfoo: 1\nbar: 2\n}\n}\n";
    // Lines 2-3 are the key-value pairs inside `inner {`.
    let edits = range_format(source, 2, 3);
    let new_text = range_new_text(&edits).expect("expected an edit");
    let lines: Vec<&str> = new_text.trim_end_matches('\n').split('\n').collect();
    for line in &lines {
        assert!(
            line.starts_with("    "),
            "expected 4-space indent (depth 2) for nested lines: {:?}",
            line
        );
    }
}

#[test]
fn range_format_strips_trailing_whitespace() {
    // The selected line has trailing spaces that should be removed.
    let source = "loop {\nfoo: 1   \nbar: 2\n}\n";
    let edits = range_format(source, 1, 1);
    let new_text = range_new_text(&edits).expect("expected an edit");
    assert!(
        !new_text.contains("  \n") && !new_text.ends_with("  "),
        "expected trailing whitespace to be stripped: {:?}",
        new_text
    );
    let line = new_text.trim_end_matches('\n');
    assert_eq!(
        line, "  foo: 1",
        "expected trimmed + indented line: {:?}",
        line
    );
}

#[test]
fn range_format_empty_source_returns_empty() {
    assert!(range_format("", 0, 5).is_empty());
}

#[test]
fn range_format_single_line() {
    // A single flush-left line inside a block should be indented.
    let source = "loop {\nfoo: 1\nbar: 2\n}\n";
    let edits = range_format(source, 1, 1);
    assert_eq!(edits.len(), 1, "expected exactly one edit: {:?}", edits);
    let new_text = range_new_text(&edits).unwrap();
    assert_eq!(
        new_text, "  foo: 1\n",
        "expected single indented line: {:?}",
        new_text
    );
}

#[test]
fn range_format_diverges_from_canonical_formatter() {
    // `range_format` runs the LSP's own line-based indentation over the
    // selected lines. The canonical `plugin-ext-fmt` engine — which is what
    // `textDocument/formatting` and the `hnl fmt` CLI use — leaves this
    // arrow-dialect snippet untouched, so the two disagree for it.
    //
    // This divergence is pinned rather than hidden: if range formatting is ever
    // delegated to the canonical engine (the fix), this assertion fails and
    // points at exactly what changed.
    let source = "loop {\nfoo: 1\nbar: 2\n}\n";
    let last_line = source.trim_end_matches('\n').split('\n').count() as u32 - 1;
    let range_result = apply_range_edits(source, &range_format(source, 0, last_line));

    assert_eq!(range_result, "loop {\n  foo: 1\n  bar: 2\n}\n");
    assert_eq!(
        handl_plugin_ext_fmt::format_source(source).unwrap(),
        source,
        "the canonical formatter is expected to leave this snippet untouched"
    );
}

#[test]
fn range_format_no_change_when_already_formatted() {
    // The selected lines are already correctly indented and trimmed, so no
    // edits should be produced.
    let source = "loop {\n  foo: 1\n  bar: 2\n}\n";
    let edits = range_format(source, 1, 2);
    assert!(
        edits.is_empty(),
        "expected no edits for already-formatted lines: {:?}",
        edits
    );
}
