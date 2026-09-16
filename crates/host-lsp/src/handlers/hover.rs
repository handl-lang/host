use std::io::Write;

use super::position_line_char;
use crate::document::{self, DocumentStore, resolve_document_text};
use crate::protocol::Message;
use crate::protocol::send_error;
use crate::protocol::send_response;
use crate::refinement::RefinementEngine;
use handl_host_tooling::ToolingHost;
use handl_host_tooling::{HoverRequest, HoverResponse};
use serde_json::{Value, json};
pub fn handle_hover(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
    host: &ToolingHost,
    refinement: &RefinementEngine,
) {
    let source = resolve_document_text(store, &msg.params);
    let uri = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("");
    let file_path = uri.strip_prefix("file://").map(|s| s.to_string());
    let (line, character) = position_line_char(&msg);
    let offset = document::position_to_offset(&source, line, character);

    // Prefer refinement hover for arrow files.
    if let Some(md) = refinement.hover_markdown(&source, file_path.as_deref()) {
        send_response(
            writer,
            id,
            json!({
                "contents": { "kind": "markdown", "value": md },
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": line, "character": character },
                }
            }),
        );
        return;
    }

    // Fall back to token-level hover via the tooling host.
    let request = HoverRequest {
        source: source.clone(),
        pos: offset,
        path: file_path.unwrap_or_default(),
    };
    match host.hover(&request) {
        Ok(HoverResponse(Some(md))) => send_response(
            writer,
            id,
            json!({
                "contents": { "kind": "markdown", "value": md }
            }),
        ),
        Ok(HoverResponse(None)) => send_response(writer, id, json!(null)),
        Err(e) => send_error(writer, id, -32603, &e.to_string()),
    }
}
