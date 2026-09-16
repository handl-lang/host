use std::io::Write;

use serde_json::{json, Value};
use handl_host_tooling::{SemanticTokensPlugin, SemanticTokensRequest};
use crate::protocol::Message;
use crate::document::{DocumentStore, resolve_document_text};
use crate::protocol::send_response;
use crate::protocol::send_error;
pub fn handle_semantic_tokens_full(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    provider: &dyn SemanticTokensPlugin,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let resp = provider
        .run(&SemanticTokensRequest { source })
        .map_err(|e| e.to_string());
    match resp {
        Ok(tokens) => {
            let data: Vec<u32> = tokens
                .0
                .iter()
                .flat_map(|t| {
                    [
                        t.delta_line,
                        t.delta_start,
                        t.length,
                        t.token_type,
                        t.token_modifiers,
                    ]
                })
                .collect();
            send_response(writer, id, json!({ "data": data }));
        }
        Err(e) => {
            send_error(writer, id, -32603, &e);
        }
    }
}

pub fn handle_semantic_tokens_range(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    provider: &dyn SemanticTokensPlugin,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let range = msg.params.as_ref().and_then(|p| p.get("range"));
    let start_line = range
        .and_then(|r| r.get("start"))
        .and_then(|s| s.get("line"))
        .and_then(|l| l.as_u64())
        .unwrap_or(0) as u32;
    let start_char = range
        .and_then(|r| r.get("start"))
        .and_then(|s| s.get("character"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as u32;
    let end_line = range
        .and_then(|r| r.get("end"))
        .and_then(|e| e.get("line"))
        .and_then(|l| l.as_u64())
        .unwrap_or(0) as u32;
    let end_char = range
        .and_then(|r| r.get("end"))
        .and_then(|e| e.get("character"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as u32;
    match provider
        .run(&SemanticTokensRequest { source })
        .map_err(|e| e.to_string())
    {
        Ok(tokens) => {
            let mut current_line: u32 = 0;
            let mut current_start: u32 = 0;
            let mut abs: Vec<(u32, u32, u32, u32, u32)> = Vec::new();
            for t in &tokens.0 {
                current_line += t.delta_line;
                if t.delta_line == 0 {
                    current_start += t.delta_start;
                } else {
                    current_start = t.delta_start;
                }
                let in_range = (current_line > start_line
                    || (current_line == start_line && current_start >= start_char))
                    && (current_line < end_line
                        || (current_line == end_line && current_start < end_char));
                if in_range {
                    abs.push((
                        current_line,
                        current_start,
                        t.length,
                        t.token_type,
                        t.token_modifiers,
                    ));
                }
            }
            let mut prev_line: u32 = 0;
            let mut prev_start: u32 = 0;
            let mut data: Vec<u32> = Vec::new();
            for (line, start, length, token_type, token_modifiers) in abs {
                let delta_line = line - prev_line;
                let delta_start = if delta_line == 0 {
                    start - prev_start
                } else {
                    start
                };
                data.push(delta_line);
                data.push(delta_start);
                data.push(length);
                data.push(token_type);
                data.push(token_modifiers);
                prev_line = line;
                prev_start = start;
            }
            send_response(writer, id, json!({ "data": data }));
        }
        Err(e) => {
            send_error(writer, id, -32603, &e);
        }
    }
}

