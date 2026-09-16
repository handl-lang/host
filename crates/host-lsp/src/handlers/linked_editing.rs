use std::io::Write;

use serde_json::{json, Value};
use crate::protocol::Message;
use crate::document::{DocumentStore, resolve_document_text};
use super::position_offset_from_msg;
use crate::protocol::send_response;
use crate::linked_editing;
pub fn handle_linked_editing(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    match linked_editing::linked_editing_ranges(&source, offset) {
        Some(v) => send_response(writer, id, v),
        None => send_response(writer, id, json!(null)),
    }
}

