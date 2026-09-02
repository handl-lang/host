//! `inlayHint/resolve` support for the handl LSP server.
//!
//! When an InlayHint is initially returned with minimal data (just a position
//! and a `data` field), the client calls `inlayHint/resolve` to fill in the
//! `tooltip` and full `label`. The `data` field carries:
//!
//! * `source` — the document source text.
//! * `property_name` — the rec state field name whose type should be shown.
//!
//! This module uses the [`RefinementEngine`] to infer the full type of the
//! property and fills in a markdown `tooltip` plus a type-annotated `label`.
//! Items without a `data` field are returned unchanged.
//!
//! [`RefinementEngine`]: crate::refinement::RefinementEngine

use serde_json::{Value, json};

use crate::refinement::RefinementEngine;

/// Resolve an InlayHint, filling in `tooltip` and `label` with full type
/// information.
///
/// `item` should be a JSON object representing an LSP InlayHint. If it carries
/// a `data` object with `source` and `property_name`, the inferred type of the
/// property is placed in `label` (as `: <type>`) and a markdown `tooltip` is
/// added; otherwise the item is returned unchanged.
pub fn resolve_inlay_hint(item: Value) -> Value {
    let mut item = item;

    let data = match item.get("data") {
        Some(d) if d.is_object() => d.clone(),
        _ => return item,
    };

    let source = data.get("source").and_then(|s| s.as_str()).unwrap_or("");
    let property_name = data
        .get("property_name")
        .and_then(|s| s.as_str())
        .unwrap_or("");

    if source.is_empty() && property_name.is_empty() {
        return item;
    }

    let engine = RefinementEngine::new();

    // Try the property-specific hover markdown first — it includes input/output
    // component types, the guarantee, and relevant loop invariants.
    if let Some(md) = engine.property_hover_markdown(source, None, property_name) {
        item["tooltip"] = json!({ "kind": "markdown", "value": md });

        // Derive a short label from the markdown's input-type line if present.
        let label = extract_short_type(&md, property_name);
        item["label"] = json!(label);
        return item;
    }

    // Fall back to whole-file inference (no per-property breakdown available).
    if let Some(Ok(result)) = engine.infer_document(source, None) {
        let label = format!(": {}", result.input.pretty());
        item["label"] = json!(label);
        item["tooltip"] = json!({
            "kind": "markdown",
            "value": format!(
                "### `{}`\n\n**Input**  `{}`\n**Output** `{}`",
                property_name,
                result.input.pretty(),
                result.output.pretty()
            ),
        });
        return item;
    }

    // No refinement available — set a placeholder tooltip so the client still
    // has something to show on hover.
    item["tooltip"] = json!({
        "kind": "markdown",
        "value": format!(
            "`{}`: type information unavailable (not an arrow file or inference failed)",
            property_name
        ),
    });
    if !property_name.is_empty() {
        item["label"] = json!(format!("{}: ?", property_name));
    }

    item
}

/// Extract a short `: <type>` label from a property hover markdown string.
///
/// Looks for the `**Input type**` line and parses the backtick-quoted type
/// after it. Falls back to `: <property_name>` when parsing fails.
fn extract_short_type(md: &str, property_name: &str) -> String {
    for line in md.lines() {
        if let Some(rest) = line.strip_prefix("**Input type**") {
            // The type is in backticks: `  `Type`` → extract inside backticks.
            if let Some(start) = rest.find('`') {
                if let Some(end) = rest[start + 1..].find('`') {
                    let ty = &rest[start + 1..start + 1 + end];
                    return format!(": {}", ty);
                }
            }
        }
    }
    if property_name.is_empty() {
        ": ?".to_string()
    } else {
        format!("{}: ?", property_name)
    }
}
