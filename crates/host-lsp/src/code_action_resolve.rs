//! `codeAction/resolve` support for the handl LSP server.
//!
//! When a CodeAction is initially returned with minimal data (just a title and
//! a `data` field), the client calls `codeAction/resolve` to fill in the
//! actual `edit`. The `data` field carries:
//!
//! * `source` — the document source text.
//! * `range` — the byte-offset range `{ "start": usize, "end": usize }` the
//!   action applies to.
//! * `action_type` — a string identifying the action (matched against the
//!   action title, e.g. `"Add missing return"`).
//!
//! This module recomputes the applicable code actions for the range and copies
//! the `edit` from the matching action into the item. Items without a `data`
//! field are returned unchanged.

#![allow(dead_code)]

use serde_json::Value;

use crate::code_actions::code_actions;

/// Resolve a CodeAction, filling in the `edit` field.
///
/// `item` should be a JSON object representing an LSP CodeAction. If it
/// carries a `data` object with `source`, `range`, and `action_type`, the
/// `edit` is computed and inserted; otherwise the item is returned unchanged.
pub fn resolve_code_action(item: Value) -> Value {
    let mut item = item;

    let data = match item.get("data") {
        Some(d) if d.is_object() => d.clone(),
        _ => return item,
    };

    let source = data.get("source").and_then(|s| s.as_str()).unwrap_or("");
    let action_type = data
        .get("action_type")
        .and_then(|s| s.as_str())
        .unwrap_or("");

    let range = data.get("range");
    let range_start = range
        .and_then(|r| r.get("start"))
        .and_then(|s| s.as_u64())
        .unwrap_or(0) as usize;
    let range_end = range
        .and_then(|r| r.get("end"))
        .and_then(|e| e.as_u64())
        .unwrap_or(0) as usize;

    if source.is_empty() && action_type.is_empty() {
        return item;
    }

    // Recompute the actions applicable to the range (no diagnostics).
    let actions = code_actions(source, range_start, range_end, &[]);

    // Find the action whose title matches `action_type`. Matching is
    // case-insensitive and substring-based so callers can pass a short label
    // like "return" or the full title like "Add missing return".
    let needle = action_type.to_lowercase();
    for action in actions {
        let title = action
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_lowercase();
        if (!needle.is_empty() && title.contains(&needle))
            || (needle.is_empty() && action.get("edit").is_some())
        {
            if let Some(edit) = action.get("edit") {
                item["edit"] = edit.clone();
            }
            return item;
        }
    }

    item
}
