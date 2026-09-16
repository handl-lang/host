use std::io::Write;

use super::position_offset_from_msg;
use crate::cross_file_rename;
use crate::document::{DocumentStore, resolve_document_text};
use crate::protocol::Message;
use crate::protocol::send_response;
use serde_json::{Value, json};
pub fn handle_prepare_rename(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    match cross_file_rename::prepare_rename(&source, offset) {
        Some(result) => send_response(writer, id, result),
        None => send_response(writer, id, json!(null)),
    }
}

pub fn handle_rename(writer: &mut impl Write, id: Value, msg: &Message, store: &DocumentStore) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let new_name = msg
        .params
        .as_ref()
        .and_then(|p| p.get("newName"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let file_path = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|u| u.strip_prefix("file://"));
    match cross_file_rename::rename(&source, offset, new_name, file_path) {
        Some(result) => send_response(writer, id, result),
        None => send_response(writer, id, json!(null)),
    }
}
