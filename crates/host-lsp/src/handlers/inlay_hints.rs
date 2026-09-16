use std::io::Write;

use serde_json::{json, Value};
use crate::protocol::Message;
use crate::document::{DocumentStore, resolve_document_text};
use crate::refinement::RefinementEngine;
use crate::protocol::send_response;
use crate::inlay_hint_resolve;
use crate::inlay_hints;
pub fn handle_inlay_hint_resolve(writer: &mut impl Write, id: Value, msg: &Message) {
    let item = msg
        .params
        .as_ref()
        .map(|p| p.clone())
        .unwrap_or(json!(null));
    let resolved = inlay_hint_resolve::resolve_inlay_hint(item);
    send_response(writer, id, resolved);
}

pub fn handle_inlay_hints(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
    refinement: &RefinementEngine,
) {
    let source = resolve_document_text(store, &msg.params);
    let file_path = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|u| u.strip_prefix("file://"));
    let hints = inlay_hints::inlay_hints(&source, refinement, file_path);
    send_response(writer, id, json!(hints));
}

