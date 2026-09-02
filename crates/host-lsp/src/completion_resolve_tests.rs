//! Tests for `crate::completion_resolve`.

use serde_json::json;

use crate::completion_resolve::resolve_completion;

/// A source with `---` doc comments (extracted by `extract_doc_comments`).
const DOC_SOURCE: &str = "--- A doc comment\n--- Second line\nx = 42";

/// A simple arrow file that refinement inference can handle.
const ARROW_SOURCE: &str = "id >>> id";

#[test]
fn resolve_completion_adds_documentation_from_doc_comments() {
    let item = json!({
        "label": "x",
        "kind": 6,
        "data": { "name": "x", "source": DOC_SOURCE },
    });
    let resolved = resolve_completion(item);
    let docs = resolved
        .get("documentation")
        .expect("documentation should be set");
    let value = docs
        .get("value")
        .and_then(|v| v.as_str())
        .expect("documentation value should be a string");
    assert!(
        value.contains("A doc comment"),
        "documentation should contain the doc comment: {}",
        value
    );
    assert_eq!(docs["kind"], "markdown");
}

#[test]
fn resolve_completion_adds_detail_from_refinement() {
    let item = json!({
        "label": "id",
        "kind": 3,
        "data": { "name": "id", "source": ARROW_SOURCE },
    });
    let resolved = resolve_completion(item);
    let detail = resolved
        .get("detail")
        .and_then(|d| d.as_str())
        .expect("detail should be set");
    assert!(
        detail.contains("~>"),
        "detail should be a type signature containing `~>`: {}",
        detail
    );
}

#[test]
fn resolve_completion_with_no_data_returns_item_unchanged() {
    let item = json!({ "label": "foo", "kind": 6 });
    let resolved = resolve_completion(item.clone());
    assert_eq!(resolved, item);
    assert!(resolved.get("documentation").is_none());
    assert!(resolved.get("detail").is_none());
}

#[test]
fn resolve_completion_empty_returns_default() {
    let item = json!({});
    let resolved = resolve_completion(item);
    // No data → returned as-is (empty object).
    assert!(resolved.get("documentation").is_none());
    assert!(resolved.get("detail").is_none());
}
