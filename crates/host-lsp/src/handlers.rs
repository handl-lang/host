use serde_json::{Value, json};

use handl_host_tooling::{SemanticTokenLegend, SemanticTokensPlugin, SemanticTokensRequest};

use crate::code_actions;
use crate::completion;
use crate::cross_file_def;
use crate::cross_file_rename;
use crate::diagnostics::parse_diagnostics;
use crate::document::{self, DocumentStore, resolve_document_text};
use crate::document_highlight;
use crate::document_symbols;
use crate::folding_ranges;
use crate::formatting;
use crate::hover;
use crate::inlay_hints;
use crate::on_type_formatting;
use crate::protocol::{Message, send_error, send_log_message, send_notification, send_response};
use crate::range_formatting;
use crate::refinement::RefinementEngine;
use crate::selection_ranges;
use crate::signature_help;
use crate::symbols;
use crate::workspace_symbols;
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
    refinement: &RefinementEngine,
) {
    let source = resolve_document_text(store, &msg.params);
    let uri = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("");
    let file_path = uri.strip_prefix("file://").map(|s| s.to_string());
    let (line, character) = position_line_char(&msg);

    // Prefer refinement hover for arrow files: the inferred contract is far
    // more useful than per-token classification for arrow-graph source.
    // But first, check if we're hovering over a property access (e.g.
    // `state.acc`) — if so, show per-property refinement info.
    let offset = document::position_to_offset(&source, line, character);

    // Check if this is a property access by looking at the token before the
    // cursor — if it's preceded by `.`, it's a property.
    if let Some(property_name) = hover::property_at_offset(&source, offset) {
        if let Some(md) =
            refinement.property_hover_markdown(&source, file_path.as_deref(), &property_name)
        {
            send_response(
                writer,
                id,
                json!({
                    "contents": { "kind": "markdown", "value": md },
                    "range": {
                        "start": { "line": line, "character": character },
                        "end": { "line": line, "character": character },
                    }
                }),
            );
            return;
        }
    }

    // Check if the cursor is on an identifier that references another arrow
    // file. If so, show that arrow's doc comments + inferred contract.
    if let Some(md) = hover::cross_arrow_hover(&source, offset, file_path.as_deref(), refinement) {
        send_response(
            writer,
            id,
            json!({
                "contents": { "kind": "markdown", "value": md },
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": line, "character": character },
                }
            }),
        );
        return;
    }

    if let Some(md) = refinement.hover_markdown(&source, file_path.as_deref()) {
        send_response(
            writer,
            id,
            json!({
                "contents": { "kind": "markdown", "value": md },
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": line, "character": character },
                }
            }),
        );
        return;
    }

    // Fall back to token-level hover for non-arrow .hnl files.
    // Pass the file path so cross-file doc comment hover can work.
    match hover::hover_at_with_path(&source, offset, file_path.as_deref()) {
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
    let offset = position_offset_from_msg(&msg, &source);
    let result = completion::completion(&source, offset);
    send_response(writer, id, result);
}

pub fn handle_signature_help(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    match signature_help::signature_help(&source, offset) {
        Some(result) => send_response(writer, id, result),
        None => send_response(writer, id, json!(null)),
    }
}

pub fn handle_document_symbols(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let symbols = document_symbols::document_symbols(&source);
    send_response(writer, id, json!(symbols));
}

pub fn handle_formatting(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let result = formatting::format_document_json(&source);
    send_response(writer, id, result);
}

pub fn handle_on_type_formatting(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let (line, character) = position_line_char(&msg);
    let ch = msg
        .params
        .as_ref()
        .and_then(|p| p.get("ch"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let edits = on_type_formatting::on_type_formatting(&source, (line, character), ch);
    send_response(writer, id, json!(edits));
}

pub fn handle_range_formatting(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let (start_line, end_line) = msg
        .params
        .as_ref()
        .and_then(|p| p.get("range"))
        .and_then(|r| {
            let start = r.get("start")?.get("line")?.as_u64()? as u32;
            let end = r.get("end")?.get("line")?.as_u64()? as u32;
            Some((start, end))
        })
        .unwrap_or((0, 0));
    let edits = range_formatting::range_format(&source, start_line, end_line);
    send_response(writer, id, json!(edits));
}

pub fn handle_folding_ranges(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let ranges = folding_ranges::folding_ranges(&source);
    send_response(writer, id, json!(ranges));
}

pub fn handle_document_highlight(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    match document_highlight::document_highlight(&source, offset) {
        Some(highlights) => send_response(writer, id, json!(highlights)),
        None => send_response(writer, id, json!(null)),
    }
}

pub fn handle_selection_ranges(
    writer: &mut impl Write,
    id: serde_json::Value,
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

pub fn handle_code_actions(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let (start_line, start_char) = msg
        .params
        .as_ref()
        .and_then(|p| p.get("range"))
        .and_then(|r| r.get("start"))
        .and_then(|s| {
            Some((
                s.get("line")?.as_u64()? as u32,
                s.get("character")?.as_u64()? as u32,
            ))
        })
        .unwrap_or((0, 0));
    let (end_line, end_char) = msg
        .params
        .as_ref()
        .and_then(|p| p.get("range"))
        .and_then(|r| r.get("end"))
        .and_then(|e| {
            Some((
                e.get("line")?.as_u64()? as u32,
                e.get("character")?.as_u64()? as u32,
            ))
        })
        .unwrap_or((0, 0));
    let range_start = document::position_to_offset(&source, start_line, start_char);
    let range_end = document::position_to_offset(&source, end_line, end_char);
    let diagnostics: Vec<Value> = msg
        .params
        .as_ref()
        .and_then(|p| p.get("context"))
        .and_then(|c| c.get("diagnostics"))
        .and_then(|d| d.as_array())
        .map(|a| a.clone())
        .unwrap_or_default();
    let actions = code_actions::code_actions(&source, range_start, range_end, &diagnostics);
    send_response(writer, id, json!(actions));
}

pub fn handle_workspace_symbols(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
) {
    let query = msg
        .params
        .as_ref()
        .and_then(|p| p.get("query"))
        .and_then(|q| q.as_str())
        .unwrap_or("");
    // Try to find workspace root from the first open document.
    let root = std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string());
    let symbols = workspace_symbols::workspace_symbols(query, root.as_deref());
    send_response(writer, id, json!(symbols));
}

pub fn handle_inlay_hints(
    writer: &mut impl Write,
    id: serde_json::Value,
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

pub fn handle_prepare_rename(
    writer: &mut impl Write,
    id: serde_json::Value,
    msg: &Message,
    store: &DocumentStore,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    match cross_file_rename::prepare_rename(&source, offset) {
        Some(result) => send_response(writer, id, result),
        None => send_response(writer, id, json!(null)),
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
    let new_name = msg
        .params
        .as_ref()
        .and_then(|p| p.get("newName"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let file_path = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|u| u.strip_prefix("file://"));
    match cross_file_rename::rename(&source, offset, new_name, file_path) {
        Some(result) => send_response(writer, id, result),
        None => send_response(writer, id, json!(null)),
    }
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

    // Try cross-file definition first (handles arrow file references like
    // `insert` in `sort.hnl` → `insert.hnl`), then fall back to in-file.
    if let Some(loc) = cross_file_def::resolve_definition(&source, offset, uri) {
        send_response(writer, id, json!([loc]));
        return;
    }

    // In-file definition (existing behavior)
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

fn position_offset_from_msg(msg: &Message, source: &str) -> usize {
    let (line, character) = position_line_char(msg);
    document::position_to_offset(source, line, character)
}

/// Extract the (line, character) position from a message's `position` field.
fn position_line_char(msg: &Message) -> (u32, u32) {
    msg.params
        .as_ref()
        .and_then(|p| p.get("position"))
        .and_then(|pos| {
            let line = pos.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
            let character = pos.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
            Some((line, character))
        })
        .unwrap_or((0, 0))
}

pub fn handle_publish_diagnostics(
    writer: &mut impl Write,
    uri: &str,
    source: &str,
    refinement: &RefinementEngine,
) {
    let mut diagnostics = Vec::new();

    // Append parse diagnostics (real syntax errors caught by the lexer and
    // the arrow-file parser) with severity 1 (Error). These surface as the
    // user types, before refinement inference runs.
    for diag in parse_diagnostics(source) {
        diagnostics.push(json!({
            "range": {
                "start": { "line": diag.line, "character": diag.character },
                "end": { "line": diag.line, "character": diag.character + 1 },
            },
            "severity": 1,
            "source": "handl-parse",
            "message": diag.message,
        }));
    }

    // Append refinement diagnostics for arrow files that fail inference.
    // Non-arrow files produce no refinement diagnostics. Refinement
    // diagnostics keep their existing severity (Warning).
    let file_path = uri.strip_prefix("file://").map(|s| s.to_string());
    for diag in refinement.diagnostics(source, file_path.as_deref()) {
        diagnostics.push(json!({
            "range": {
                "start": { "line": diag.line, "character": diag.character },
                "end": { "line": diag.line, "character": diag.character + 1 },
            },
            "severity": 2,
            "source": "handl-refinement",
            "message": diag.message,
        }));
    }

    let params = json!({ "uri": uri, "diagnostics": diagnostics });
    send_notification(writer, "textDocument/publishDiagnostics", params);
}

/// Handle `textDocument/codeLens`: show the inferred refinement contract
/// summary as a code lens at the top of arrow files.
pub fn handle_code_lens(
    writer: &mut impl Write,
    id: serde_json::Value,
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
            "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
            "definitionProvider": true,
            "referencesProvider": true,
            "renameProvider": { "prepareProvider": true },
            "codeLensProvider": true,
            "documentSymbolProvider": true,
            "documentFormattingProvider": true,
            "documentRangeFormattingProvider": true,
            "documentOnTypeFormattingProvider": {
                "firstTriggerCharacter": "\n",
                "moreTriggerCharacter": ["}", ")", "]"]
            },
            "foldingRangeProvider": true,
            "documentHighlightProvider": true,
            "selectionRangeProvider": true,
            "workspaceSymbolProvider": true,
            "codeActionProvider": true,
            "inlayHintProvider": true,
        },
        "serverInfo": {
            "name": "handl-lsp",
            "version": "0.1.0",
        }
    })
}
