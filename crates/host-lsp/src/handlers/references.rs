use std::io::Write;

use super::position_offset_from_msg;
use crate::document::{DocumentStore, resolve_document_text};
use crate::protocol::Message;
use crate::protocol::send_response;
use crate::symbols;
use serde_json::{Value, json};
pub fn handle_references(writer: &mut impl Write, id: Value, msg: &Message, store: &DocumentStore) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let uri = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("");
    match symbols::identifier_at_offset(&source, offset) {
        Some((name, _)) => {
            let locations: Vec<Value> = symbols::find_references(&source, &name)
                .into_iter()
                .map(|span| {
                    json!({
                        "uri": uri,
                        "range": symbols::span_to_range(&source, span),
                    })
                })
                .collect();
            send_response(writer, id, json!(locations));
        }
        None => send_response(writer, id, json!([])),
    }
}
