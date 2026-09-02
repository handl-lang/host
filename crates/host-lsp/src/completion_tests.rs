use serde_json::json;

use crate::completion::{completion, completions};
use crate::document::DocumentStore;
use crate::handlers::handle_completion;
use crate::protocol::Message;

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").unwrap() + 4;
    s[header_end..].to_string()
}

/// Number of keyword entries in the KEYWORDS const (v0.1 surface syntax).
const KEYWORD_COUNT: usize = 19;

#[test]
fn completions_empty_source_returns_only_keywords() {
    let items = completions("");
    // Empty source: keywords only, no arrow snippets (doesn't look like arrow file), no identifiers.
    assert_eq!(items.len(), KEYWORD_COUNT);
    let match_item = items
        .iter()
        .find(|i| i["label"] == "match")
        .expect("match keyword present");
    // match has a snippet, so kind should be SNIPPET (15)
    assert_eq!(match_item["kind"], 15);
    assert!(match_item["detail"].as_str().unwrap().contains("match"));
}

#[test]
fn completions_includes_identifiers_classified_by_role() {
    let items = completions("x = 42; y = x; foo(a: 1)");
    // keywords + identifiers (x, y, foo, a)
    let x = items.iter().find(|i| i["label"] == "x").expect("x present");
    // x is followed by `=`, so it's a binding (VARIABLE=6)
    assert_eq!(x["kind"], 6);
    assert_eq!(x["detail"], "binding");

    let foo = items
        .iter()
        .find(|i| i["label"] == "foo")
        .expect("foo present");
    // foo is followed by `(`, so it's a function (FUNCTION=3)
    assert_eq!(foo["kind"], 3);
    assert_eq!(foo["detail"], "function");

    // Each identifier appears once (deduplicated)
    let x_count = items.iter().filter(|i| i["label"] == "x").count();
    assert_eq!(x_count, 1, "x should appear once");
}

#[test]
fn completions_lex_error_still_returns_keywords() {
    let items = completions("x = \"unterminated");
    assert_eq!(items.len(), KEYWORD_COUNT);
    assert!(items.iter().any(|i| i["label"] == "match"));
}

#[test]
fn completions_arrow_file_includes_arrow_snippets() {
    let items = completions("id >>> id");
    // keywords + arrow snippets + identifiers
    let seq = items
        .iter()
        .find(|i| i["label"] == ">>>")
        .expect(">>> snippet present");
    assert_eq!(seq["kind"], 15); // SNIPPET
    assert!(seq["detail"].as_str().unwrap().contains(">>>"));
}

#[test]
fn completions_non_arrow_file_no_arrow_snippets() {
    let items = completions("x = 42");
    // No arrow snippets for non-arrow files
    assert!(!items.iter().any(|i| i["label"] == ">>>" && i["kind"] == 15));
    assert!(!items.iter().any(|i| i["label"] == "~>" && i["kind"] == 15));
}

#[test]
fn completions_rec_keyword_has_snippet() {
    let items = completions("");
    let rec_item = items
        .iter()
        .find(|i| i["label"] == "rec")
        .expect("rec keyword present");
    assert_eq!(rec_item["kind"], 15); // SNIPPET
    let insert = rec_item["insertText"].as_str().unwrap();
    assert!(insert.contains("rec"));
    assert!(insert.contains("state"));
}

#[test]
fn completions_boolean_literals_are_constant_kind() {
    let items = completions("");
    let true_item = items
        .iter()
        .find(|i| i["label"] == "true")
        .expect("true present");
    assert_eq!(true_item["kind"], 21); // CONSTANT
}

#[test]
fn handle_completion_writes_response_with_array() {
    let mut store = DocumentStore::new();
    store.open("file:///test.hnl", "x = 42");

    let mut buf = Vec::new();
    let msg = Message {
        method: "textDocument/completion".to_string(),
        params: Some(json!({
            "textDocument": { "uri": "file:///test.hnl" },
            "position": { "line": 0, "character": 0 }
        })),
        id: Some(json!(1)),
    };

    handle_completion(&mut buf, json!(1), &msg, &store);

    let parsed: serde_json::Value = serde_json::from_str(&extract_body(&buf)).unwrap();
    assert_eq!(parsed["id"], 1);
    let items = parsed["result"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i["label"] == "match"));
    assert!(items.iter().any(|i| i["label"] == "x"));
}

// ---------------------------------------------------------------------------
// Context-aware completion tests
// ---------------------------------------------------------------------------

/// Helper: find an item by label within a JSON array Value.
fn find_item<'a>(items: &'a [serde_json::Value], label: &str) -> Option<&'a serde_json::Value> {
    items.iter().find(|i| i["label"] == label)
}

#[test]
#[ignore = "kit lexer (old grammar) does not recognise `rec` as a keyword; \
            detect_context never reaches AfterRec until the lexer is updated to v0.1"]
fn completion_after_rec_suggests_brace() {
    // Cursor right after the `rec` keyword.
    let src = "rec ";
    // The token preceding the cursor is the `rec` keyword (its span ends at 4).
    let result = completion(src, 4);
    let items = result.as_array().expect("array");
    let brace = find_item(items, "{").expect("brace snippet suggested after rec");
    assert_eq!(brace["kind"], 15); // SNIPPET
    assert!(brace["detail"].as_str().unwrap().contains("rec state"));
}

#[test]
fn completion_after_match_suggests_scrutinee() {
    let src = "match ";
    let result = completion(src, 6);
    let items = result.as_array().expect("array");
    let item = find_item(items, "expr {").expect("expr { suggested after match");
    assert_eq!(item["kind"], 15);
    let insert = item["insertText"].as_str().unwrap();
    assert!(insert.contains("${1:expr}"));
    assert!(insert.contains("[]"));
    assert!(insert.contains("[h, ...t]"));
}

#[test]
fn completion_after_do_suggests_brace() {
    let src = "do ";
    let result = completion(src, 3);
    let items = result.as_array().expect("array");
    let item = find_item(items, "{").expect("brace suggested after do");
    assert_eq!(item["kind"], 15);
    assert!(item["detail"].as_str().unwrap().contains("do-while"));
}

#[test]
#[ignore = "kit lexer (old grammar) does not recognise `return` as a keyword; \
            detect_context never reaches AfterReturn until the lexer is updated to v0.1"]
fn completion_after_return_suggests_expr() {
    let src = "return ";
    let result = completion(src, 7);
    let items = result.as_array().expect("array");
    let item = find_item(items, "expr").expect("expr suggested after return");
    assert_eq!(item["kind"], 15);
    let insert = item["insertText"].as_str().unwrap();
    assert!(insert.contains("${1:expr}"));
}

#[test]
#[ignore = "kit lexer (old grammar) does not recognise `rec` as a keyword, so \
            rec_state_fields never collects fields; needs lexer v0.1 update"]
fn completion_after_state_dot_suggests_rec_fields() {
    // A rec state declares `remaining` and `acc`; cursor after `state.`.
    let src = "rec { remaining = input, acc = [] } match state.";
    let offset = src.len();
    let result = completion(src, offset);
    let items = result.as_array().expect("array");
    let remaining = items
        .iter()
        .find(|i| i["label"] == "remaining" && i["detail"] == "rec state field")
        .expect("remaining field suggested after state.");
    assert_eq!(remaining["kind"], 8); // PROPERTY
    let acc = items
        .iter()
        .find(|i| i["label"] == "acc" && i["detail"] == "rec state field")
        .expect("acc field suggested after state.");
    assert_eq!(acc["kind"], 8);
}

#[test]
#[ignore = "kit lexer (old grammar) does not recognise `rec` as a keyword, so \
            rec_state_fields never collects fields; needs lexer v0.1 update"]
fn completion_after_rec_brace_suggests_fields() {
    // Cursor right after the `{` of a rec state block with no fields yet —
    // context is StateField but there is nothing to suggest (no crash).
    let src = "rec { ";
    let offset = src.len();
    let result = completion(src, offset);
    let items = result.as_array().expect("array");
    assert!(items.iter().any(|i| i["label"] == "rec"));

    // A second rec block reuses fields declared in the first one: cursor
    // after the second `rec {` suggests the in-scope field names.
    let src2 = "rec { remaining = input, acc = [] } rec { ";
    let result2 = completion(src2, src2.len());
    let items2 = result2.as_array().expect("array");
    assert!(items2
        .iter()
        .any(|i| i["label"] == "remaining" && i["detail"] == "rec state field"));
    assert!(items2
        .iter()
        .any(|i| i["label"] == "acc" && i["detail"] == "rec state field"));
}

#[test]
fn completion_after_equal_in_do_suggests_all_combinators() {
    // In v0.1, arrow bindings use `=` instead of `<-`. Inside a `do {}` block,
    // an `=` triggers the ArrowExpr context, offering v0.1 combinators.
    let src = "do {\n  y = ";
    let offset = src.len();
    let result = completion(src, offset);
    let items = result.as_array().expect("array");
    for comb in [">>>", "***", "&&&", "|||", "id"] {
        let item = items
            .iter()
            .find(|i| i["label"] == comb && i["kind"] == 15)
            .unwrap_or_else(|| panic!("combinator {} suggested after = in do", comb));
        assert!(item["detail"].as_str().unwrap().contains("arrow"));
    }
}

#[test]
fn completion_after_equal_in_do_suggests_combinators() {
    let src = "do {\n  x = ";
    let offset = src.len();
    let result = completion(src, offset);
    let items = result.as_array().expect("array");
    assert!(items
        .iter()
        .any(|i| i["label"] == ">>>" && i["kind"] == 15));
    assert!(items
        .iter()
        .any(|i| i["label"] == "id" && i["kind"] == 15));
}

#[test]
fn completion_at_do_line_start_suggests_combinators() {
    // Cursor after the opening `{` of a do-block (start of first statement).
    let src = "do { ";
    let offset = src.len();
    let result = completion(src, offset);
    let items = result.as_array().expect("array");
    assert!(items
        .iter()
        .any(|i| i["label"] == ">>>" && i["kind"] == 15));
}

#[test]
fn completion_after_equal_in_plain_binding_no_combinators() {
    // A plain `x = ` binding (not an arrow context) should NOT add combinator
    // snippets beyond the base set.
    let src = "x = ";
    let offset = src.len();
    let result = completion(src, offset);
    let items = result.as_array().expect("array");
    // `id` and `>>>` are combinators that only appear via context-aware logic.
    assert!(!items.iter().any(|i| i["label"] == "id" && i["kind"] == 15));
    assert!(!items.iter().any(|i| i["label"] == ">>>" && i["kind"] == 15));
}

#[test]
fn completion_after_match_pattern_colon_suggests_patterns() {
    let src = "rec { remaining = input, acc = [] } match state.remaining {\n  []: ";
    let offset = src.len();
    let result = completion(src, offset);
    let items = result.as_array().expect("array");
    let empty = find_item(items, "[]").expect("[] pattern suggested");
    assert_eq!(empty["kind"], 15);
    assert!(empty["insertText"].as_str().unwrap().contains("[]: ${1:body}"));
    let nonempty = find_item(items, "[h, ...t]").expect("[h, ...t] pattern suggested");
    assert_eq!(nonempty["kind"], 15);
    assert!(nonempty["insertText"].as_str().unwrap().contains("[h, ...t]: ${1:body}"));
}

#[test]
fn completion_no_context_returns_full_set() {
    // Cursor at the very start: no preceding token → full completion set.
    let src = "x = 42";
    let result = completion(src, 0);
    let items = result.as_array().expect("array");
    assert!(items.iter().any(|i| i["label"] == "match"));
    assert!(items.iter().any(|i| i["label"] == "x"));
}

#[test]
fn completion_lex_error_falls_back_to_full_set() {
    let src = "x = \"unterminated";
    let result = completion(src, src.len());
    let items = result.as_array().expect("array");
    assert!(items.iter().any(|i| i["label"] == "match"));
    assert_eq!(items.len(), KEYWORD_COUNT); // KEYWORD_COUNT only (lex error → no identifiers)
}
