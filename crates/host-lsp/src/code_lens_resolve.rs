//! `codeLens/resolve` support for the handl LSP server.
//!
//! When a CodeLens is initially returned with minimal data (just a range and a
//! `data` field), the client calls `codeLens/resolve` to fill in the
//! `command`. The `data` field carries:
//!
//! * `source` — the document source text.
//! * `file_path` — the filesystem path of the document (used for cross-file
//!   reference resolution; may be omitted).
//!
//! This module uses the [`RefinementEngine`] to infer the contract and formats
//! it as the lens's `command.title`. Items without a `data` field are returned
//! unchanged.
//!
//! [`RefinementEngine`]: crate::refinement::RefinementEngine

use serde_json::{Value, json};

use crate::refinement::{RefinementEngine, format_refinement_summary};

/// Resolve a CodeLens, filling in the `command` field with the inferred
/// contract display.
///
/// `item` should be a JSON object representing an LSP CodeLens. If it carries
/// a `data` object with a `source` string, the inferred contract summary is
/// placed in `command.title`; otherwise the item is returned unchanged.
pub fn resolve_code_lens(item: Value) -> Value {
    let mut item = item;

    let data = match item.get("data") {
        Some(d) if d.is_object() => d.clone(),
        _ => return item,
    };

    let source = data.get("source").and_then(|s| s.as_str()).unwrap_or("");
    let file_path = data.get("file_path").and_then(|s| s.as_str());

    if source.is_empty() {
        return item;
    }

    let engine = RefinementEngine::new();

    let title = match engine.infer_document(source, file_path) {
        Some(Ok(result)) => format_refinement_summary(&result),
        // Not an arrow file or inference failed — leave a placeholder so the
        // client still renders something rather than an empty lens.
        _ => "handl: no contract inferred".to_string(),
    };

    item["command"] = json!({
        "title": title,
        "command": "handl.showContract",
    });

    item
}
