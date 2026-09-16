use std::io::Write;

use crate::document::{DocumentStore, resolve_document_text};
use crate::protocol::{Message, send_response, send_error};
use handl_host_tooling::{ToolingHost, lsp::FoldingRange};
use serde_json::{Value, json};

pub fn handle_folding_ranges(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
    host: &ToolingHost,
) {
    let source = resolve_document_text(store, &msg.params);

    match host.folding_ranges(&source) {
        Ok(ranges) => {
            let result: Vec<Value> = ranges
                .iter()
                .map(|r: &FoldingRange| {
                    json!({
                        "startLine": r.start_line,
                        "endLine": r.end_line,
                        "startCharacter": r.start_char,
                        "endCharacter": r.end_char,
                        "kind": r.kind,
                    })
                })
                .collect();
            send_response(writer, id, Value::Array(result));
        }
        Err(e) => {
            let message = e.to_string();
            send_error(writer, id, -32603, &message);
        }
    }
}
