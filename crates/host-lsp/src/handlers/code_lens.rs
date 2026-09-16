use std::io::Write;

use serde_json::{json, Value};
use crate::protocol::Message;
use crate::document::{DocumentStore, resolve_document_text};
use crate::refinement::RefinementEngine;
use crate::protocol::send_response;
/// Handle `textDocument/codeLens`: show the inferred refinement contract
/// summary as a code lens at the top of arrow files.
pub fn handle_code_lens(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
    refinement: &RefinementEngine,
) {
    let source = resolve_document_text(store, &msg.params);
    let lenses: Vec<Value> = refinement
        .code_lenses(&source)
        .into_iter()
        .map(|lens| {
            json!({
                "range": {
                    "start": { "line": lens.line, "character": lens.character },
                    "end": { "line": lens.line, "character": lens.character },
                },
                "command": {
                    "title": lens.title,
                    "command": "",
                }
            })
        })
        .collect();
    send_response(writer, id, json!(lenses));
}

