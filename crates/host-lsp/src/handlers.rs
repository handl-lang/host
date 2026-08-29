use serde_json::{Value, json};

use handl_host_tooling::{SemanticTokenLegend, SemanticTokensPlugin, SemanticTokensRequest};

use crate::diagnostics::collect_diagnostics;
use crate::document::{self, DocumentStore, resolve_document_text};
use crate::hover;
use crate::protocol::{Message, send_error, send_notification, send_response};
use std::io::Write;

pub fn handle_initialize(writer: &mut impl Write, id: serde_json::Value, legend: &SemanticTokenLegend) {
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
                "range": false,
            },
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": { "includeText": true }
            },
            "hoverProvider": true,
        },
        "serverInfo": {
            "name": "handl-lsp",
            "version": "0.1.0",
        }
    })
}
