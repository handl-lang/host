//! `completionItem/resolve` support for the handl LSP server.
//!
//! When a [`completion`] item is initially returned with minimal data, the
//! client calls `completionItem/resolve` to fill in additional details. The
//! item carries a `data` field (a JSON object) containing the source text and
//! the identifier name; this module uses that data to enrich the item with:
//!
//! * `documentation` — a markdown string with the doc comment (if any) and/or
//!   the inferred refinement contract.
//! * `detail` — a short type signature string (e.g. `In ~> Out`).
//!
//! Items without a `data` field are returned unchanged.
//!
//! [`completion`]: crate::completion

use serde_json::{Value, json};

use crate::hover::extract_doc_comments;
use crate::refinement::RefinementEngine;

/// Resolve a CompletionItem, adding `documentation` and `detail`.
///
/// `item` should be a JSON object representing an LSP CompletionItem. If it
/// carries a `data` object with `name` and `source` string fields, the item is
/// enriched; otherwise it is returned unchanged.
pub fn resolve_completion(item: Value) -> Value {
    let mut item = item;

    // Need a `data` object to resolve.
    let data = match item.get("data") {
        Some(d) if d.is_object() => d.clone(),
        _ => return item,
    };

    let source = data.get("source").and_then(|s| s.as_str()).unwrap_or("");
    let name = data.get("name").and_then(|s| s.as_str()).unwrap_or("");

    if source.is_empty() && name.is_empty() {
        return item;
    }

    let engine = RefinementEngine::new();

    // Build the documentation: prefer doc comments, then fall back to the
    // inferred refinement contract (if the source is an arrow file).
    let mut documentation = String::new();
    if let Some(docs) = extract_doc_comments(source) {
        documentation.push_str(&docs);
    }

    let mut detail = String::new();

    if let Some(Ok(result)) = engine.infer_document(source, None) {
        // Short type signature: `In ~> Out`.
        detail = format!("{} ~> {}", result.input.pretty(), result.output.pretty());

        // If there were no doc comments, use the inferred contract as docs.
        if documentation.is_empty() {
            documentation.push_str("### Inferred Contract\n\n");
            documentation.push_str(&format!("**Input**  `{}`\n\n", result.input.pretty()));
            documentation.push_str(&format!("**Output** `{}`\n\n", result.output.pretty()));
            documentation.push_str(&format!(
                "**Guarantee** `{}`\n",
                result.guarantee.simplify().pretty()
            ));
        }
    }

    // Fall back to a name-based detail if refinement produced nothing.
    if detail.is_empty() {
        detail = if name.is_empty() {
            "handl".to_string()
        } else {
            name.to_string()
        };
    }

    if !documentation.is_empty() {
        item["documentation"] = json!({
            "kind": "markdown",
            "value": documentation,
        });
    }
    item["detail"] = json!(detail);

    item
}
