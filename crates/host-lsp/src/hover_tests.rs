use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use crate::document::DocumentStore;
use crate::handlers::handle_hover;
use crate::hover::{extract_doc_comments, hover_at, hover_at_with_path};
use crate::protocol::Message;
use crate::refinement::RefinementEngine;

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").unwrap() + 4;
    s[header_end..].to_string()
}

fn contents_value(value: &serde_json::Value) -> &serde_json::Value {
    if value["contents"].is_string() {
        &value["contents"]
    } else {
        &value["contents"]["value"]
    }
}

#[test]
fn hover_identifier_shows_role() {
    let value = hover_at("x = 42", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    // x is followed by `=`, so it should be classified as a binding
    assert!(contents.contains("binding"), "contents: {contents}");
    assert!(contents.contains("x"), "contents: {contents}");
}

#[test]
fn hover_number_at_value() {
    let value = hover_at("x = 42", 4).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("number"), "contents: {contents}");
    assert!(contents.contains("42"), "contents: {contents}");
}

#[test]
fn hover_keyword_shows_synopsis_and_syntax() {
    let value = hover_at("match", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Pattern-match"), "contents: {contents}");
    assert!(contents.contains("match"), "contents: {contents}");
    // Should include a syntax hint in a code block
    assert!(contents.contains("```"), "contents: {contents}");
}

#[test]
fn hover_if_keyword_shows_conditional_syntax() {
    let value = hover_at("if x { 1 } else { 2 }", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Conditional"), "contents: {contents}");
    assert!(contents.contains("if `cond` { `a` } else { `b` }"), "contents: {contents}");
}

#[test]
fn hover_rec_keyword_shows_arrow_syntax() {
    let value = hover_at("rec", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Feedback loop"), "contents: {contents}");
    assert!(contents.contains("rec"), "contents: {contents}");
}

#[test]
fn hover_function_call_classified_as_function() {
    let value = hover_at("foo(1, 2)", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("function"), "contents: {contents}");
    assert!(contents.contains("foo"), "contents: {contents}");
}

#[test]
fn hover_property_in_braces_classified_as_property() {
    let src = "{ name: \"alice\", age: 30 }";
    let value = hover_at(src, 2).unwrap(); // hover on "name"
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("property"), "contents: {contents}");
}

#[test]
fn hover_parameter_in_parens_classified_as_parameter() {
    let src = "foo(a: 1, b: 2)";
    let value = hover_at(src, 4).unwrap(); // hover on "a"
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("parameter"), "contents: {contents}");
}

#[test]
fn hover_pascalcase_classified_as_type() {
    let value = hover_at("Flag", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("type"), "contents: {contents}");
}

#[test]
fn hover_operator_shows_name() {
    let value = hover_at("x = 42", 2).unwrap(); // hover on "="
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("assignment"), "contents: {contents}");
}

#[test]
fn hover_whitespace_returns_none() {
    assert!(hover_at("x = 42", 1).is_none());
}

#[test]
fn hover_empty_source_returns_none() {
    assert!(hover_at("", 0).is_none());
}

#[test]
fn hover_shows_occurrence_count_for_repeated_identifier() {
    let value = hover_at("x = 1; y = x; z = x", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("3 occurrences"), "contents: {contents}");
}

#[test]
fn handle_hover_writes_response_with_contents() {
    let engine = RefinementEngine::new();
    let mut store = DocumentStore::new();
    store.open("file:///test.hnl", "x = 42");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/hover".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.hnl" },
            "position": { "line": 0, "character": 0 }
        })),
        id: Some(json!(1)),
    };

    handle_hover(&mut buf, json!(1), &msg, &store, &engine);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 1);
    let contents = parsed["result"]["contents"]["value"].as_str().unwrap();
    assert!(contents.contains("binding"));
}

#[test]
fn handle_hover_returns_null_in_whitespace() {
    let engine = RefinementEngine::new();
    let mut store = DocumentStore::new();
    store.open("file:///test.hnl", "x = 42");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/hover".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.hnl" },
            "position": { "line": 0, "character": 1 }
        })),
        id: Some(json!(2)),
    };

    handle_hover(&mut buf, json!(2), &msg, &store, &engine);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 2);
    assert_eq!(parsed["result"], serde_json::Value::Null);
}

#[test]
fn hover_open_bracket_shows_array() {
    let value = hover_at("[1, 2, 3]", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Array"), "contents: {contents}");
    assert!(contents.contains("3 elements"), "contents: {contents}");
}

#[test]
fn hover_empty_brackets_shows_empty_list() {
    let value = hover_at("[]", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("empty list"), "contents: {contents}");
}

#[test]
fn hover_open_brace_shows_record_with_field_count() {
    let value = hover_at("{ a: 1, b: 2, c: 3 }", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Record"), "contents: {contents}");
    assert!(contents.contains("3 fields"), "contents: {contents}");
}

#[test]
fn hover_empty_braces_shows_empty_record() {
    let value = hover_at("{}", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("empty record"), "contents: {contents}");
}

#[test]
fn hover_spread_operator() {
    let value = hover_at("[1, ...rest]", 5).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Spread"), "contents: {contents}");
}

#[test]
fn hover_dot_accessor_with_property() {
    let value = hover_at("state.acc", 5).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Property access"), "contents: {contents}");
}

#[test]
fn hover_dot_accessor_with_method_call() {
    let value = hover_at("arr.map(f)", 3).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Method call"), "contents: {contents}");
}

#[test]
fn hover_property_access_identifier_shows_property_role() {
    let value = hover_at("state.acc", 6).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("property"), "contents: {contents}");
}

#[test]
fn hover_method_call_identifier_shows_method_role() {
    let value = hover_at("arr.map(f)", 5).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("method"), "contents: {contents}");
}

#[test]
fn hover_paren_shows_group_or_call() {
    let value = hover_at("foo(1, 2)", 3).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Group"), "contents: {contents}");
    assert!(contents.contains("2 arguments"), "contents: {contents}");
}

// --- Arrow combinator hovers ---

#[test]
fn hover_arr_combinator() {
    let value = hover_at("arr(\"id\")", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Arrow combinator: `arr`"), "contents: {contents}");
    assert!(contents.contains("Lift a named pure function"), "contents: {contents}");
}

#[test]
fn hover_comp_combinator() {
    let value = hover_at("comp(a, b)", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("sequential composition"), "contents: {contents}");
    assert!(contents.contains("a >>> b"), "contents: {contents}");
}

#[test]
fn hover_first_combinator() {
    let value = hover_at("first(id)", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Arrow combinator: `first`"), "contents: {contents}");
    assert!(contents.contains("fan-out"), "contents: {contents}");
}

#[test]
fn hover_choice_combinator() {
    let value = hover_at("choice(l, r)", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Arrow combinator: `choice`"), "contents: {contents}");
    assert!(contents.contains("Either"), "contents: {contents}");
}

#[test]
fn hover_app_combinator() {
    let value = hover_at("app(\"f\", x)", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Arrow combinator: `app`"), "contents: {contents}");
    assert!(contents.contains("higher-order"), "contents: {contents}");
}

#[test]
fn hover_id_combinator() {
    let value = hover_at("id", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Arrow combinator: `id`"), "contents: {contents}");
    assert!(contents.contains("Identity"), "contents: {contents}");
}

#[test]
fn hover_insert_arrow() {
    let value = hover_at("insert({ elem: h, acc: state.acc })", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("Arrow: `insert`"), "contents: {contents}");
    assert!(contents.contains("sorted list"), "contents: {contents}");
}

#[test]
fn hover_non_combinator_function_still_shows_function_role() {
    let value = hover_at("myFunc(1, 2)", 0).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("function"), "contents: {contents}");
    assert!(!contents.contains("Arrow combinator"), "should not be combinator: {contents}");
}

// --- Doc comment hover tests ---

#[test]
fn extract_doc_comments_from_arrow_file() {
    let source = "--- sort: insertion sort over a list.\n---\n--- Written as an arrow graph.\nrec { } (id)";
    let docs = extract_doc_comments(source).unwrap();
    assert!(docs.contains("sort: insertion sort"), "docs: {docs}");
    assert!(docs.contains("Written as an arrow graph"), "docs: {docs}");
}

#[test]
fn extract_doc_comments_returns_none_for_no_docs() {
    let source = "rec { } (id)";
    assert!(extract_doc_comments(source).is_none());
}

#[test]
fn extract_doc_comments_skips_regular_comments() {
    // Regular `--` comments before `---` should be skipped, not included.
    let source = "-- regular comment\n--- doc comment\nrec { } (id)";
    let docs = extract_doc_comments(source).unwrap();
    assert!(docs.contains("doc comment"), "docs: {docs}");
    assert!(!docs.contains("regular comment"), "docs: {docs}");
}

#[test]
fn extract_doc_comments_strips_prefix_and_space() {
    let source = "---  indented doc\nrec { } (id)";
    let docs = extract_doc_comments(source).unwrap();
    // Should strip `---` prefix. The lexer's delimited close includes the
    // trailing newline, and we strip one leading space.
    assert_eq!(docs.trim(), "indented doc");
}

#[test]
fn hover_rec_keyword_shows_file_doc_comments() {
    let source = "--- my arrow: does stuff.\n---\n--- More details here.\nrec { } (id)";
    // Hover over the `rec` keyword (offset after the doc comments).
    let rec_offset = source.find("rec").unwrap();
    let value = hover_at_with_path(source, rec_offset, None).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("my arrow"), "contents: {contents}");
    assert!(contents.contains("More details"), "contents: {contents}");
}

#[test]
fn hover_rec_keyword_without_doc_comments_falls_back() {
    let source = "rec { } (id)";
    let rec_offset = source.find("rec").unwrap();
    let value = hover_at_with_path(source, rec_offset, None).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    // Should fall back to identifier hover (rec is not a keyword in the old
    // lexer), not doc comment hover.
    assert!(contents.contains("rec"), "contents: {contents}");
    assert!(!contents.contains("---"), "should not have doc separator: {contents}");
}

/// Monotonic counter for unique temp directories in hover tests.
static HOVER_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn hover_arrow_reference_shows_cross_file_docs() {
    // Create a unique temp arrows directory with a target arrow file.
    let n = HOVER_TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp = std::env::temp_dir().join(format!("handl_hover_doc_test_{}", n));
    std::fs::create_dir_all(temp.join("handl-packages").join("kit-stdlib").join("fn")).unwrap();
    let target = temp.join("handl-packages").join("kit-stdlib").join("fn").join("myarrow.hnl");
    std::fs::write(&target, "--- myarrow: a test arrow.\n---\n--- It does things.\nrec { } (id)").unwrap();

    // The current file references `myarrow` — hover over it.
    let source = "myarrow(state)";
    let current_file = temp.join("handl-packages").join("kit-stdlib").join("caller.hnl");
    std::fs::write(&current_file, source).unwrap();
    let file_path = current_file.to_string_lossy().to_string();

    let value = hover_at_with_path(source, 0, Some(&file_path)).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    assert!(contents.contains("myarrow"), "contents: {contents}");
    assert!(contents.contains("a test arrow"), "contents: {contents}");
    assert!(contents.contains("It does things"), "contents: {contents}");

    // Cleanup
    std::fs::remove_dir_all(&temp).ok();
}

#[test]
fn hover_property_access_does_not_trigger_doc_hover() {
    // `state.myarrow` — property access, not an arrow reference.
    let value = hover_at("state.myarrow", 6).unwrap();
    let contents = contents_value(&value).as_str().unwrap();
    // Should show property hover, not doc comment hover.
    assert!(contents.contains("property"), "contents: {contents}");
}
