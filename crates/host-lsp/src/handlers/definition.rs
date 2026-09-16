use std::io::Write;

use super::position_offset_from_msg;
use crate::document::{DocumentStore, resolve_document_text};
use crate::implementation;
use crate::line_map::LineMap;
use crate::protocol::Message;
use crate::protocol::send_error;
use crate::protocol::send_response;
use crate::refinement::RefinementEngine;
use crate::type_definition;
use handl_host_tooling::GotoDefRequest;
use handl_host_tooling::ToolingHost;
use serde_json::{Value, json};
pub fn handle_type_definition(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
    refinement: &RefinementEngine,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let file_path = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|s| s.strip_prefix("file://"));
    match type_definition::type_definition(&source, offset, file_path, refinement) {
        Some(loc) => send_response(writer, id, loc),
        None => send_response(writer, id, json!(null)),
    }
}

pub fn handle_implementation(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let file_path = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|s| s.strip_prefix("file://"));
    let locs = implementation::implementation(&source, offset, file_path);
    send_response(writer, id, json!(locs));
}

pub fn handle_definition(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
    host: &ToolingHost,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let uri = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("");
    let file_path = uri.strip_prefix("file://").unwrap_or(uri);

    let request = GotoDefRequest {
        source,
        pos: offset,
        path: file_path.to_string(),
    };

    match host.goto_def(&request) {
        Ok(Some(loc)) => {
            let lines = LineMap::new(&request.source);
            let (start_line, start_char) = lines.position(loc.start);
            let (end_line, end_char) = lines.position(loc.end);
            let range = json!({
                "start": { "line": start_line, "character": start_char },
                "end": { "line": end_line, "character": end_char },
            });
            let target_uri = if let Some(uri) = &loc.uri {
                uri.clone()
            } else {
                uri.to_string()
            };
            send_response(writer, id, json!([{ "uri": target_uri, "range": range }]));
        }
        Ok(None) => {
            send_response(writer, id, json!(null));
        }
        Err(e) => {
            let msg = e.to_string();
            send_error(writer, id, -32603, &msg);
        }
    }
}
