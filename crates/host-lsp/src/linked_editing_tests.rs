use crate::linked_editing::linked_editing_ranges;

#[test]
fn linked_editing_on_open_brace_returns_matching_close() {
    // `{ ... }` — cursor on the opening brace should link to the closing one.
    let source = "foo = { a: 1 }";
    let offset = source.find('{').unwrap();
    let result = linked_editing_ranges(source, offset).expect("should link braces");
    let ranges = result
        .get("ranges")
        .and_then(|r| r.as_array())
        .expect("ranges array");
    assert_eq!(ranges.len(), 2, "expected two linked ranges: {result:?}");

    // The second range should be the closing brace at the end of the source.
    let close = &ranges[1];
    let end_char = close["end"]["character"].as_u64().unwrap();
    assert_eq!(
        end_char,
        source.len() as u64,
        "close brace range end should be source end"
    );
}

#[test]
fn linked_editing_on_close_paren_returns_matching_open() {
    // Cursor on the closing paren should link back to the opening paren.
    let source = "bar = ( x )";
    let offset = source.find(')').unwrap();
    let result = linked_editing_ranges(source, offset).expect("should link parens");
    let ranges = result.get("ranges").and_then(|r| r.as_array()).unwrap();
    assert_eq!(ranges.len(), 2, "expected two linked ranges: {result:?}");
    // First range is the cursor's own token (the close paren).
    let first_start = ranges[0]["start"]["character"].as_u64().unwrap();
    assert_eq!(first_start, offset as u64);
}

#[test]
fn linked_editing_on_identifier_returns_all_occurrences() {
    // `sort = ... sort` — cursor on `sort` links every occurrence.
    let source = "sort = 1; sort";
    let offset = source.find("sort").unwrap();
    let result = linked_editing_ranges(source, offset).expect("should link identifier");
    let ranges = result.get("ranges").and_then(|r| r.as_array()).unwrap();
    assert_eq!(
        ranges.len(),
        2,
        "expected two occurrences of `sort`: {result:?}"
    );
}

#[test]
fn linked_editing_on_whitespace_returns_none() {
    let source = "x = 42";
    let offset = source.find(' ').unwrap();
    assert!(linked_editing_ranges(source, offset).is_none());
}

#[test]
fn linked_editing_unmatched_bracket_links_only_itself() {
    // An unmatched `[` has no partner — link only to itself.
    let source = "x = [ 1";
    let offset = source.find('[').unwrap();
    let result = linked_editing_ranges(source, offset).expect("should still return a range");
    let ranges = result.get("ranges").and_then(|r| r.as_array()).unwrap();
    assert_eq!(
        ranges.len(),
        1,
        "unmatched bracket links only itself: {result:?}"
    );
}
