use std::io::Write;

use serde_json::{json, Value};
use crate::protocol::Message;
use crate::document::{DocumentStore, resolve_document_text};
use super::position_offset_from_msg;
use crate::protocol::send_response;
use crate::call_hierarchy;
pub fn handle_prepare_call_hierarchy(
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
    let items = call_hierarchy::prepare_call_hierarchy(&source, offset, file_path);
    send_response(writer, id, json!(items));
}

pub fn handle_call_hierarchy_incoming(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
) {
    let item = msg.params.as_ref().and_then(|p| p.get("item"));
    let name = item
        .and_then(|i| i.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let file_path = item
        .and_then(|i| i.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|s| s.strip_prefix("file://"));
    let calls = call_hierarchy::incoming_calls(name, file_path);
    send_response(writer, id, json!(calls));
}

pub fn handle_call_hierarchy_outgoing(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
) {
    let item = msg.params.as_ref().and_then(|p| p.get("item"));
    let name = item
        .and_then(|i| i.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let file_path = item
        .and_then(|i| i.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|s| s.strip_prefix("file://"));
    let calls = call_hierarchy::outgoing_calls(name, file_path);
    send_response(writer, id, json!(calls));
}

