use std::io::Write;

use serde_json::{json, Value};
use crate::protocol::Message;
use crate::document::{DocumentStore, resolve_document_text};
use super::position_line_char;
use crate::protocol::send_response;
use crate::formatting;
use crate::on_type_formatting;
use crate::range_formatting;
pub fn handle_formatting(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let result = formatting::format_document_json(&source);
    send_response(writer, id, result);
}

pub fn handle_on_type_formatting(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let (line, character) = position_line_char(&msg);
    let ch = msg
        .params
        .as_ref()
        .and_then(|p| p.get("ch"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let edits = on_type_formatting::on_type_formatting(&source, (line, character), ch);
    send_response(writer, id, json!(edits));
}

pub fn handle_range_formatting(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let (start_line, end_line) = msg
        .params
        .as_ref()
        .and_then(|p| p.get("range"))
        .and_then(|r| {
            let start = r.get("start")?.get("line")?.as_u64()? as u32;
            let end = r.get("end")?.get("line")?.as_u64()? as u32;
            Some((start, end))
        })
        .unwrap_or((0, 0));
    let edits = range_formatting::range_format(&source, start_line, end_line);
    send_response(writer, id, json!(edits));
}

