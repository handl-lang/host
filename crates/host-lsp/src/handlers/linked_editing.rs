use std::io::Write;

use super::position_offset_from_msg;
use crate::document::{DocumentStore, resolve_document_text};
use crate::linked_editing;
use crate::protocol::Message;
use crate::protocol::send_response;
use serde_json::{Value, json};
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
