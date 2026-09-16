use std::io::Write;

use serde_json::{json, Value};
use crate::protocol::Message;
use crate::document::{DocumentStore, resolve_document_text};
use super::position_offset_from_msg;
use crate::protocol::send_response;
use crate::signature_help;
pub fn handle_signature_help(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    match signature_help::signature_help(&source, offset) {
        Some(result) => send_response(writer, id, result),
        None => send_response(writer, id, json!(null)),
    }
}

