use std::io::Write;

use super::position_offset_from_msg;
use crate::document::{DocumentStore, resolve_document_text};
use crate::document_highlight;
use crate::protocol::Message;
use crate::protocol::send_response;
use serde_json::{Value, json};
pub fn handle_document_highlight(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    match document_highlight::document_highlight(&source, offset) {
        Some(highlights) => send_response(writer, id, json!(highlights)),
        None => send_response(writer, id, json!(null)),
    }
}
