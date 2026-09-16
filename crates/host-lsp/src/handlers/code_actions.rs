use std::io::Write;

use serde_json::{json, Value};
use crate::protocol::Message;
use crate::document::{self, DocumentStore, resolve_document_text};
use crate::protocol::send_response;
use crate::code_actions;
pub fn handle_code_actions(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let (start_line, start_char) = msg
        .params
        .as_ref()
        .and_then(|p| p.get("range"))
        .and_then(|r| r.get("start"))
        .and_then(|s| {
            Some((
                s.get("line")?.as_u64()? as u32,
                s.get("character")?.as_u64()? as u32,
            ))
        })
        .unwrap_or((0, 0));
    let (end_line, end_char) = msg
        .params
        .as_ref()
        .and_then(|p| p.get("range"))
        .and_then(|r| r.get("end"))
        .and_then(|e| {
            Some((
                e.get("line")?.as_u64()? as u32,
                e.get("character")?.as_u64()? as u32,
            ))
        })
        .unwrap_or((0, 0));
    let range_start = document::position_to_offset(&source, start_line, start_char);
    let range_end = document::position_to_offset(&source, end_line, end_char);
    let diagnostics: Vec<Value> = msg
        .params
        .as_ref()
        .and_then(|p| p.get("context"))
        .and_then(|c| c.get("diagnostics"))
        .and_then(|d| d.as_array())
        .map(|a| a.clone())
        .unwrap_or_default();
    let actions = code_actions::code_actions(&source, range_start, range_end, &diagnostics);
    send_response(writer, id, json!(actions));
}

