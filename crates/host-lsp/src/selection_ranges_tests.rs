use crate::selection_ranges::selection_ranges;

/// Walk the `parent` chain of a SelectionRange and collect every `range`
/// object (innermost first).
fn collect_ranges(sel: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut cur = Some(sel);
    while let Some(node) = cur {
        if node.get("range").is_none() {
            break;
        }
        out.push(node["range"].clone());
        cur = node.get("parent").filter(|p| !p.is_null());
    }
    out
}

/// Return the byte width of a range via its start/end line+character, assuming
/// a single-line source so character offsets map directly to bytes.
fn range_chars(r: &serde_json::Value) -> (u64, u64) {
    let s = r["start"]["character"].as_u64().unwrap_or(0);
    let e = r["end"]["character"].as_u64().unwrap_or(0);
    (s, e)
}

#[test]
fn selection_ranges_returns_token_level_at_cursor() {
    // `loop { x }` — cursor on the `x` token.
    let src = "loop { x }";
    let pos = src.find('x').unwrap();
    let result = selection_ranges(src, &[pos]);
    assert_eq!(result.len(), 1, "one position → one linked list");
    let ranges = collect_ranges(&result[0]);
    assert!(!ranges.is_empty(), "should have at least one range");
    // First (innermost) range covers exactly the `x` token.
    let (s, e) = range_chars(&ranges[0]);
    assert_eq!(s, pos as u64, "token range start");
    assert_eq!(e, (pos + 1) as u64, "token range end");
}

#[test]
fn selection_ranges_expands_to_bracket_content() {
    // `loop { x }` — cursor on `x`. The second level should cover the content
    // inside the braces (` x `).
    let src = "loop { x }";
    let pos = src.find('x').unwrap();
    let ranges = collect_ranges(&selection_ranges(src, &[pos])[0]);
    assert!(ranges.len() >= 2, "need at least token + bracket content");
    // Inner bracket content starts right after `{` and ends right before `}`.
    let open_end = src.find('{').unwrap() + 1;
    let close_start = src.find('}').unwrap();
    let (s, e) = range_chars(&ranges[1]);
    assert_eq!(s, open_end as u64, "bracket content start");
    assert_eq!(e, close_start as u64, "bracket content end");
}

#[test]
fn selection_ranges_expands_to_bracket_pair() {
    // `loop { x }` — cursor on `x`. The third level should cover the braces
    // themselves (`{ x }`).
    let src = "loop { x }";
    let pos = src.find('x').unwrap();
    let ranges = collect_ranges(&selection_ranges(src, &[pos])[0]);
    assert!(ranges.len() >= 3, "need at least token + content + pair");
    let open_start = src.find('{').unwrap();
    let close_end = src.find('}').unwrap() + 1;
    let (s, e) = range_chars(&ranges[2]);
    assert_eq!(s, open_start as u64, "bracket pair start");
    assert_eq!(e, close_end as u64, "bracket pair end");
}

#[test]
fn selection_ranges_expands_to_line() {
    // A single-line source where the line is wider than the brackets.
    let src = "loop { x }";
    let pos = src.find('x').unwrap();
    let ranges = collect_ranges(&selection_ranges(src, &[pos])[0]);
    // Some level should cover the entire line [0, src.len()).
    let has_line = ranges.iter().any(|r| {
        let (s, e) = range_chars(r);
        s == 0 && e == src.len() as u64
    });
    assert!(
        has_line,
        "expected a level covering the whole line, got {ranges:?}"
    );
}

#[test]
fn selection_ranges_expands_to_document() {
    // Multi-line source so the document level is strictly wider than any line.
    let src = "loop {\n  x\n}\n";
    let pos = src.find('x').unwrap();
    let ranges = collect_ranges(&selection_ranges(src, &[pos])[0]);
    assert!(!ranges.is_empty(), "should have ranges");
    // The outermost (last) range must cover the entire document.
    let last = ranges.last().unwrap();
    let (s_line, s_char) = (
        last["start"]["line"].as_u64().unwrap(),
        last["start"]["character"].as_u64().unwrap(),
    );
    let (e_line, e_char) = (
        last["end"]["line"].as_u64().unwrap(),
        last["end"]["character"].as_u64().unwrap(),
    );
    assert_eq!(s_line, 0, "document start line");
    assert_eq!(s_char, 0, "document start char");
    // End should reach the final line of the source.
    let last_line = src.matches('\n').count() as u64;
    assert_eq!(e_line, last_line, "document end line, got {e_line}");
    assert_eq!(e_char, 0, "document end char");
}

#[test]
fn selection_ranges_handles_nested_brackets() {
    // Nested brackets: `loop { state ~> { y } }` with the cursor on `y`.
    let src = "loop { state ~> { y } }";
    let pos = src.find('y').unwrap();
    let ranges = collect_ranges(&selection_ranges(src, &[pos])[0]);
    // We expect: token `y`, inner-brace content, inner braces, outer-brace
    // content, outer braces, line, document — i.e. several levels.
    assert!(
        ranges.len() >= 4,
        "nested brackets should yield multiple levels, got {ranges:?}"
    );
    // The innermost range is the `y` token.
    let (s, e) = range_chars(&ranges[0]);
    assert_eq!(s, pos as u64);
    assert_eq!(e, (pos + 1) as u64);
    // Some level should cover the inner `{ y }` braces. The inner `{` is the
    // last `{` in the source; its matching `}` is the first `}` after it.
    let inner_open = src.rfind('{').unwrap();
    let inner_close = src[inner_open..].find('}').unwrap() + inner_open + 1;
    let has_inner = ranges.iter().any(|r| {
        let (s, e) = range_chars(r);
        s == inner_open as u64 && e == inner_close as u64
    });
    assert!(has_inner, "expected a level for the inner braces");
    // Some level should cover the outer `loop { ... }` braces. The outer `{`
    // is the first `{`; its matching `}` is the last `}` in the source.
    let outer_open = src.find('{').unwrap();
    let outer_close = src.rfind('}').unwrap() + 1;
    let has_outer = ranges.iter().any(|r| {
        let (s, e) = range_chars(r);
        s == outer_open as u64 && e == outer_close as u64
    });
    assert!(has_outer, "expected a level for the outer braces");
}

#[test]
fn selection_ranges_empty_source_returns_empty() {
    let result = selection_ranges("", &[0]);
    assert_eq!(result.len(), 1, "one position → one entry");
    // For empty source the entry is a null/placeholder — no real range.
    assert!(
        result[0].is_null() || result[0].get("range").is_none(),
        "empty source should not produce a real range, got {result:?}"
    );
}

#[test]
fn selection_ranges_multiple_positions() {
    let src = "loop { x }";
    let pos_a = src.find('x').unwrap();
    let pos_b = src.find("loop").unwrap();
    let result = selection_ranges(src, &[pos_a, pos_b]);
    assert_eq!(result.len(), 2, "two positions → two linked lists");
    // Each entry should be a valid SelectionRange with at least one range.
    for entry in &result {
        let ranges = collect_ranges(entry);
        assert!(!ranges.is_empty(), "each position should yield ranges");
    }
    // The first list's innermost range is the `x` token.
    let a_ranges = collect_ranges(&result[0]);
    let (s, e) = range_chars(&a_ranges[0]);
    assert_eq!(s, pos_a as u64);
    assert_eq!(e, (pos_a + 1) as u64);
    // The second list's innermost range is the `loop` token.
    let b_ranges = collect_ranges(&result[1]);
    let (s, e) = range_chars(&b_ranges[0]);
    assert_eq!(s, pos_b as u64);
    assert_eq!(e, (pos_b + 4) as u64);
}

#[test]
fn selection_ranges_returns_valid_json() {
    let src = "loop { state ~> { y } }";
    let pos = src.find('y').unwrap();
    let result = selection_ranges(src, &[pos]);
    assert_eq!(result.len(), 1);
    let entry = &result[0];
    // Must have a top-level "range".
    assert!(entry.get("range").is_some(), "missing top-level range");
    // Each node in the chain must have a "range" with start/end line+character.
    let mut cur = Some(entry);
    while let Some(node) = cur {
        let range = node
            .get("range")
            .expect("every SelectionRange node must have a range");
        assert!(range.get("start").is_some(), "range missing start");
        assert!(range.get("end").is_some(), "range missing end");
        assert!(
            range["start"].get("line").is_some()
                && range["start"].get("character").is_some(),
            "start missing line/character"
        );
        assert!(
            range["end"].get("line").is_some()
                && range["end"].get("character").is_some(),
            "end missing line/character"
        );
        cur = node.get("parent").filter(|p| !p.is_null());
    }
}
