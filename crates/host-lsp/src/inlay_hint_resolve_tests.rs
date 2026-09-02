//! Tests for `crate::inlay_hint_resolve`.

use serde_json::json;

use crate::inlay_hint_resolve::resolve_inlay_hint;

/// A simple arrow file that refinement inference can handle.
const ARROW_SOURCE: &str = "id >>> id";

/// A non-arrow source (refinement inference returns `None`).
const NON_ARROW_SOURCE: &str = "x = 42";

#[test]
fn resolve_inlay_hint_adds_tooltip_and_label() {
    let item = json!({
        "position": { "line": 0, "character": 3 },
        "kind": 1,
        "data": { "source": ARROW_SOURCE, "property_name": "acc" },
    });
    let resolved = resolve_inlay_hint(item);
    // A tooltip should always be set when data is present.
    let tooltip = resolved
        .get("tooltip")
        .expect("tooltip should be set by resolve");
    assert_eq!(tooltip["kind"], "markdown");
    let value = tooltip
        .get("value")
        .and_then(|v| v.as_str())
        .expect("tooltip should have a markdown value");
    assert!(
        !value.is_empty(),
        "tooltip should not be empty: {}",
        value
    );
    // A label should be set.
    let label = resolved
        .get("label")
        .and_then(|l| l.as_str())
        .expect("label should be set by resolve");
    assert!(
        !label.is_empty(),
        "label should not be empty: {}",
        label
    );
}

#[test]
fn resolve_inlay_hint_non_arrow_source_sets_placeholder() {
    let item = json!({
        "position": { "line": 0, "character": 3 },
        "data": { "source": NON_ARROW_SOURCE, "property_name": "x" },
    });
    let resolved = resolve_inlay_hint(item);
    let tooltip = resolved
        .get("tooltip")
        .and_then(|t| t.get("value"))
        .and_then(|v| v.as_str())
        .expect("placeholder tooltip should be set");
    assert!(
        tooltip.contains("unavailable"),
        "non-arrow source should get a placeholder tooltip: {}",
        tooltip
    );
    let label = resolved
        .get("label")
        .and_then(|l| l.as_str())
        .expect("label should be set");
    assert!(label.contains("x"), "label should mention the property: {}", label);
}

#[test]
fn resolve_inlay_hint_with_no_data_returns_item_unchanged() {
    let item = json!({
        "position": { "line": 0, "character": 3 },
        "label": ": Int",
        "kind": 1,
    });
    let resolved = resolve_inlay_hint(item.clone());
    assert_eq!(resolved, item);
    assert!(resolved.get("tooltip").is_none());
}

#[test]
fn resolve_inlay_hint_empty_returns_default() {
    let item = json!({});
    let resolved = resolve_inlay_hint(item);
    assert!(resolved.get("tooltip").is_none());
    assert!(resolved.get("label").is_none());
}
