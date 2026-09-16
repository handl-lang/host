use std::io::Write;

use crate::document::{self, DocumentStore, resolve_document_text};
use crate::protocol::Message;
use crate::protocol::send_response;
use crate::selection_ranges;
use serde_json::{Value, json};
pub fn handle_selection_ranges(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let positions: Vec<usize> = msg
        .params
        .as_ref()
        .and_then(|p| p.get("positions"))
        .and_then(|ps| ps.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|pos| {
                    let line = pos.get("line")?.as_u64()? as u32;
                    let character = pos.get("character")?.as_u64()? as u32;
                    Some(document::position_to_offset(&source, line, character))
                })
                .collect()
        })
        .unwrap_or_default();
    let ranges = selection_ranges::selection_ranges(&source, &positions);
    send_response(writer, id, json!(ranges));
}
