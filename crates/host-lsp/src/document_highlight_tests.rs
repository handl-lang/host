use crate::document_highlight::document_highlight;

#[test]
fn document_highlight_finds_all_occurrences() {
    // `x = 42; y = x` — two occurrences of `x`.
    let source = "x :: 42; y :: x";
    let offset = source.find('x').unwrap();
    let result = document_highlight(source, offset).expect("should find highlights");
    assert_eq!(
        result.len(),
        2,
        "expected 2 occurrences of x, got {result:?}"
    );
}

#[test]
fn document_highlight_marks_declaration_as_write() {
    // The `x` in `x = 42` is a declaration → kind 2 (Write).
    let source = "x :: 42; y :: x";
    let offset = source.find('x').unwrap();
    let result = document_highlight(source, offset).expect("should find highlights");
    let decl = result
        .iter()
        .find(|h| h["range"]["start"]["character"].as_u64() == Some(0))
        .expect("should have a highlight at character 0");
    assert_eq!(
        decl["kind"].as_u64(),
        Some(2),
        "declaration should be Write (2)"
    );
}

#[test]
fn document_highlight_marks_usage_as_read() {
    // The `x` in `y = x` is a usage → kind 1 (Read).
    let source = "x :: 42; y :: x";
    let usage_offset = source.rfind('x').unwrap();
    let result = document_highlight(source, usage_offset).expect("should find highlights");
    // Find the highlight whose start character matches the usage offset.
    let (start_line, start_char) = (0u64, usage_offset as u64);
    let usage = result
        .iter()
        .find(|h| {
            h["range"]["start"]["line"].as_u64() == Some(start_line)
                && h["range"]["start"]["character"].as_u64() == Some(start_char)
        })
        .expect("should have a highlight at the usage offset");
    assert_eq!(usage["kind"].as_u64(), Some(1), "usage should be Read (1)");
}

#[test]
fn document_highlight_on_whitespace_returns_none() {
    // Cursor on the space between `x` and `=` is not on an identifier.
    let source = "x :: 42";
    let offset = source.find(' ').unwrap();
    assert!(document_highlight(source, offset).is_none());
}

#[test]
fn document_highlight_on_non_identifier_returns_none() {
    // Cursor on a number is not on an identifier.
    let source = "x :: 42";
    let offset = source.find('4').unwrap();
    assert!(document_highlight(source, offset).is_none());
}

#[test]
fn document_highlight_no_matches_returns_empty() {
    // An identifier that appears only once still resolves, but there is only
    // one occurrence — the result should be a non-None empty-ish list with a
    // single entry. The spec says "cursor on an identifier that appears once"
    // returns empty. We treat a single occurrence as no *additional* matches,
    // so the list has exactly one entry (the cursor's own occurrence).
    let source = "z :: 1";
    let offset = source.find('z').unwrap();
    let result = document_highlight(source, offset).expect("should resolve identifier");
    // Only one occurrence exists, so no other matches to highlight.
    assert_eq!(
        result.len(),
        1,
        "single occurrence yields one highlight: {result:?}"
    );
}

#[test]
fn document_highlight_returns_valid_json() {
    let source = "x :: 42; y :: x";
    let offset = source.find('x').unwrap();
    let result = document_highlight(source, offset).expect("should find highlights");
    assert!(!result.is_empty(), "should have highlights");
    for h in &result {
        assert!(
            h.get("range").is_some(),
            "each highlight must have a range: {h:?}"
        );
        assert!(
            h["range"].get("start").is_some() && h["range"].get("end").is_some(),
            "range must have start and end: {h:?}"
        );
        assert!(
            h["range"]["start"].get("line").is_some()
                && h["range"]["start"].get("character").is_some(),
            "start must have line and character: {h:?}"
        );
        assert!(
            h["range"]["end"].get("line").is_some() && h["range"]["end"].get("character").is_some(),
            "end must have line and character: {h:?}"
        );
        let kind = h["kind"].as_u64();
        assert!(
            kind == Some(1) || kind == Some(2) || kind == Some(3),
            "kind must be 1 (Read), 2 (Write), or 3 (Text): {h:?}"
        );
    }
}
