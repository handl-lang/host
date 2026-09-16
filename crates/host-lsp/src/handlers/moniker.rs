use std::io::Write;

use super::position_offset_from_msg;
use crate::document::{DocumentStore, resolve_document_text};
use crate::moniker;
use crate::protocol::Message;
use crate::protocol::send_response;
use serde_json::{Value, json};
pub fn handle_moniker(writer: &mut impl Write, id: Value, msg: &Message, store: &DocumentStore) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let file_path = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|s| s.strip_prefix("file://"));
    let monikers = moniker::moniker(&source, offset, file_path);
    send_response(writer, id, json!(monikers));
}
