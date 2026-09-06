//! Tests for `crate::code_action_resolve`.

use serde_json::json;

use crate::code_action_resolve::resolve_code_action;

/// A match expression with an arm body that is a bare object literal `{ … }`
/// (no `return` prefix) — this triggers the "Add missing return" quick fix.
const MATCH_SOURCE: &str = "match x {\n  []: { foo }\n}";

#[test]
fn resolve_code_action_adds_edit() {
    let item = json!({
        "title": "Add missing return",
        "kind": "quickfix",
        "data": {
            "source": MATCH_SOURCE,
            "range": { "start": 0, "end": MATCH_SOURCE.len() },
            "action_type": "Add missing return",
        },
    });
    let resolved = resolve_code_action(item);
    let edit = resolved
        .get("edit")
        .expect("edit should be filled in by resolve");
    let changes = edit.get("changes").expect("edit should contain changes");
    // The "Add missing return" action inserts `return ` before the `{`.
    let any_new_text = changes
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(|edits| edits.as_array().into_iter().flatten())
        .filter_map(|e| e.get("newText").and_then(|t| t.as_str()))
        .next()
        .unwrap_or("");
    assert!(
        any_new_text.contains("return"),
        "edit should insert `return`: newText = {:?}",
        any_new_text
    );
}

#[test]
fn resolve_code_action_with_no_data_returns_item_unchanged() {
    let item = json!({ "title": "Wrap in `do` block", "kind": "quickfix" });
    let resolved = resolve_code_action(item.clone());
    assert_eq!(resolved, item);
    assert!(resolved.get("edit").is_none());
}

#[test]
fn resolve_code_action_empty_returns_default() {
    let item = json!({});
    let resolved = resolve_code_action(item);
    assert!(resolved.get("edit").is_none());
}

#[test]
fn resolve_code_action_no_matching_action_leaves_edit_absent() {
    let item = json!({
        "title": "Nonexistent action",
        "data": {
            "source": "x = 42",
            "range": { "start": 0, "end": 6 },
            "action_type": "does not exist",
        },
    });
    let resolved = resolve_code_action(item);
    // No matching action → no edit added.
    assert!(
        resolved.get("edit").is_none(),
        "no edit should be added when no action matches"
    );
}
