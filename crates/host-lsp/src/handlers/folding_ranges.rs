use std::io::Write;

use crate::document::{DocumentStore, resolve_document_text};
use crate::folding_ranges;
use crate::protocol::Message;
use crate::protocol::send_response;
use serde_json::{Value, json};
pub fn handle_folding_ranges(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let ranges = folding_ranges::folding_ranges(&source);
    send_response(writer, id, json!(ranges));
}
