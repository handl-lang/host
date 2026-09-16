use std::io::Write;

use serde_json::Value;
use handl_host_tooling::ToolingHost;
use crate::protocol::Message;
use crate::document::{DocumentStore, resolve_document_text};
use crate::line_map::LineMap;
use crate::protocol::send_response;
use crate::protocol::send_error;
pub fn handle_document_symbols(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
    host: &ToolingHost,
) {
    let source = resolve_document_text(store, &msg.params);
    let lines = LineMap::new(&source);
    match host.document_symbols(&source) {
        Ok(symbols) => {
            let to_position = |offset: usize| {
                let (l, c) = lines.position(offset);
                crate::lsp_types::Position {
                    line: l as u32,
                    character: c as u32,
                }
            };
            let to_range = |start: usize, end: usize| crate::lsp_types::Range {
                start: to_position(start),
                end: to_position(end),
            };
            let result: Vec<crate::lsp_types::DocumentSymbol> = symbols
                .into_iter()
                .map(|s| crate::lsp_types::DocumentSymbol {
                    name: s.name,
                    kind: s.kind,
                    detail: s.detail,
                    range: to_range(s.range_start, s.range_end),
                    selection_range: to_range(s.selection_start, s.selection_end),
                })
                .collect();
            send_response(writer, id, serde_json::to_value(&result).unwrap());
        }
        Err(e) => {
            let msg = e.to_string();
            send_error(writer, id, -32603, &msg);
        }
    }
}

