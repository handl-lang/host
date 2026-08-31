use serde_json::{Value, json};

use handl_host_tooling::{SemanticTokenLegend, SemanticTokensPlugin, SemanticTokensRequest};

use crate::completion;
use crate::diagnostics::collect_diagnostics;
use crate::document::{self, DocumentStore, resolve_document_text};
use crate::hover;
use crate::protocol::{Message, send_error, send_log_message, send_notification, send_response};
use crate::symbols;
use std::io::Write;

pub fn handle_initialize(
    writer: &mut impl Write,
    id: serde_json::Value,
    legend: &SemanticTokenLegend,
) {
    let result = build_initialize_result(legend);
    send_response(writer, id, result);
}

pub fn handle_semantic_tokens_full(
    writer: &mut impl Write,
    id: serde_json::Value,
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
    id: serde_json::Value,
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

pub fn handle_shutdown(writer: &mut impl Write, id: serde_json::Value) {
    send_response(writer, id, json!(null));
}

pub fn handle_hover(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let (line, character) = msg
        .params
        .as_ref()
        .and_then(|p| p.get("position"))
        .and_then(|pos| {
            let line = pos.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
            let character = pos.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
            Some((line, character))
        })
        .unwrap_or((0, 0));
    let offset = document::position_to_offset(&source, line, character);
    match hover::hover_at(&source, offset) {
        Some(value) => send_response(writer, id, value),
        None => send_response(writer, id, json!(null)),
    }
}

pub fn handle_unknown(writer: &mut impl Write, id: Value) {
    send_error(writer, id, -32601, "method not found");
}

#[allow(dead_code)]
pub fn handle_log(writer: &mut impl Write, level: u32, message: &str) {
    send_log_message(writer, level, message);
}

pub fn handle_completion(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let items = completion::completions(&source);
    send_response(writer, id, json!(items));
}

pub fn handle_definition(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let uri = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("");
    match symbols::identifier_at_offset(&source, offset)
        .and_then(|(name, _)| symbols::find_definition(&source, &name))
    {
        Some(span) => {
            let locations = json!([{
                "uri": uri,
                "range": symbols::span_to_range(&source, span),
            }]);
            send_response(writer, id, locations);
        }
        None => send_response(writer, id, json!(null)),
    }
}

pub fn handle_references(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
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

pub fn handle_rename(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let uri = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("");
    let new_name = msg
        .params
        .as_ref()
        .and_then(|p| p.get("newName"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    match symbols::identifier_at_offset(&source, offset) {
        Some((name, _)) if !new_name.is_empty() => {
            let edits: Vec<Value> = symbols::find_references(&source, &name)
                .into_iter()
                .map(|span| {
                    json!({
                        "range": symbols::span_to_range(&source, span),
                        "newText": new_name,
                    })
                })
                .collect();
            let workspace_edit = json!({ "changes": { uri: edits } });
            send_response(writer, id, workspace_edit);
        }
        _ => send_response(writer, id, json!(null)),
    }
}

fn position_offset_from_msg(msg: &Message, source: &str) -> usize {
    let (line, character) = msg
        .params
        .as_ref()
        .and_then(|p| p.get("position"))
        .and_then(|pos| {
            let line = pos.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
            let character = pos.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
            Some((line, character))
        })
        .unwrap_or((0, 0));
    document::position_to_offset(source, line, character)
}

pub fn handle_publish_diagnostics(writer: &mut impl Write, uri: &str, source: &str) {
    let diagnostics = collect_diagnostics(source);
    let params = json!({ "uri": uri, "diagnostics": diagnostics });
    send_notification(writer, "textDocument/publishDiagnostics", params);
}

pub fn build_initialize_result(legend: &SemanticTokenLegend) -> Value {
    json!({
        "capabilities": {
            "semanticTokensProvider": {
                "legend": {
                    "tokenTypes": legend.token_types.iter().map(|t| {
                        json!({"name": t})
                    }).collect::<Vec<_>>(),
                    "tokenModifiers": legend.token_modifiers.iter().map(|m| {
                        json!({"name": m})
                    }).collect::<Vec<_>>(),
                },
                "full": true,
                "range": true,
            },
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": { "includeText": true }
            },
            "hoverProvider": true,
            "completionProvider": { "triggerCharacters": [] },
            "definitionProvider": true,
            "referencesProvider": true,
            "renameProvider": { "prepareProvider": false },
        },
        "serverInfo": {
            "name": "handl-lsp",
            "version": "0.1.0",
        }
    })
}
