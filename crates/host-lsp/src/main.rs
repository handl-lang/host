mod code_action_resolve;
mod code_lens_resolve;
mod call_hierarchy;
mod code_actions;
mod completion;
mod completion_resolve;
mod cross_file_def;
mod cross_file_rename;
mod diagnostics;
mod document;
mod document_highlight;
mod document_link;
mod document_symbols;
mod execute_command;
mod file_watcher;
mod folding_ranges;
mod formatting;
mod handlers;
mod on_type_formatting;
mod hover;
mod implementation;
mod inlay_hints;
mod inlay_hint_resolve;
mod line_map;
mod linked_editing;
mod moniker;
mod protocol;
mod range_formatting;
mod refinement;
mod selection_ranges;
mod signature_help;
mod symbols;
mod type_definition;
mod workspace_symbols;

#[cfg(test)]
mod call_hierarchy_tests;
#[cfg(test)]
mod cancellation_tests;
#[cfg(test)]
mod code_action_resolve_tests;
#[cfg(test)]
mod code_actions_tests;
#[cfg(test)]
mod code_lens_resolve_tests;
#[cfg(test)]
mod completion_resolve_tests;
#[cfg(test)]
mod completion_tests;
#[cfg(test)]
mod cross_file_def_tests;
#[cfg(test)]
mod cross_file_rename_tests;
#[cfg(test)]
mod diagnostics_tests;
#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod document_highlight_tests;
#[cfg(test)]
mod document_link_tests;
#[cfg(test)]
mod execute_command_tests;
#[cfg(test)]
mod file_watcher_tests;
#[cfg(test)]
mod folding_ranges_tests;
#[cfg(test)]
mod formatting_tests;
#[cfg(test)]
mod handlers_tests;
#[cfg(test)]
mod on_type_formatting_tests;
#[cfg(test)]
mod hover_tests;
#[cfg(test)]
mod inlay_hints_tests;
#[cfg(test)]
mod implementation_tests;
#[cfg(test)]
mod inlay_hint_resolve_tests;
#[cfg(test)]
mod linked_editing_tests;
#[cfg(test)]
mod moniker_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod range_formatting_tests;
#[cfg(test)]
mod refinement_tests;
#[cfg(test)]
mod selection_ranges_tests;
#[cfg(test)]
mod signature_help_tests;
#[cfg(test)]
mod symbols_tests;
#[cfg(test)]
mod type_definition_tests;
#[cfg(test)]
mod document_symbols_tests;
#[cfg(test)]
mod workspace_symbols_tests;

use std::collections::HashSet;
use std::io::{self, BufRead};

use handl_host_tooling::{SemanticTokensPlugin, TextContentChange, canonical_semantic_tokens};

use document::DocumentStore;
use handlers::{
    handle_code_actions, handle_code_lens, handle_completion, handle_completion_resolve,
    handle_definition, handle_document_highlight, handle_document_symbols,
    handle_execute_command, handle_folding_ranges, handle_formatting, handle_hover,
    handle_implementation, handle_inlay_hints, handle_inlay_hint_resolve,
    handle_initialize, handle_linked_editing, handle_moniker, handle_on_type_formatting,
    handle_prepare_call_hierarchy, handle_call_hierarchy_incoming,
    handle_call_hierarchy_outgoing, handle_document_link, handle_prepare_rename,
    handle_publish_diagnostics, handle_range_formatting,
    handle_references, handle_rename, handle_selection_ranges, handle_semantic_tokens_full,
    handle_semantic_tokens_range, handle_signature_help, handle_shutdown, handle_type_definition,
    handle_unknown, handle_workspace_symbols,
};
use protocol::{Message, send_log_message};
use refinement::RefinementEngine;

fn read_message(reader: &mut impl BufRead) -> Option<Message> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n');
        if trimmed.is_empty() {
            break;
        }
        if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
            content_length = len_str.parse().ok();
        }
    }

    let len = content_length?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    let body = String::from_utf8(buf).ok()?;
    serde_json::from_str(&body).ok()
}

fn main() {
    let tokens_provider = canonical_semantic_tokens();
    let legend = tokens_provider.legend();
    let refinement_engine = RefinementEngine::new();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut reader = stdin.lock();
    let mut store = DocumentStore::new();
    let mut cancelled: HashSet<serde_json::Value> = HashSet::new();

    while let Some(msg) = read_message(&mut reader) {
        let id = msg.id.clone().unwrap_or(serde_json::Value::Null);

        if msg.id.is_some() && is_cancelled(&id, &cancelled) {
            cancelled.remove(&id);
            send_log_message(&mut writer, 4, &format!("cancelled: {}", id));
            continue;
        }

        match msg.method.as_str() {
            "initialize" => {
                handle_initialize(&mut writer, id, &legend);
                send_log_message(&mut writer, 3, "handl-lsp initialized");
            }
            "initialized" => {}
            "textDocument/semanticTokens/full" => {
                handle_semantic_tokens_full(&mut writer, id, &msg, &tokens_provider, &store);
            }
            "textDocument/semanticTokens/range" => {
                handle_semantic_tokens_range(&mut writer, id, &msg, &tokens_provider, &store);
            }
            "textDocument/hover" => {
                handle_hover(&mut writer, id, &msg, &store, &refinement_engine);
            }
            "textDocument/completion" => {
                handle_completion(&mut writer, id, &msg, &store);
            }
            "textDocument/signatureHelp" => {
                handle_signature_help(&mut writer, id, &msg, &store);
            }
            "textDocument/codeLens" => {
                handle_code_lens(&mut writer, id, &msg, &store, &refinement_engine);
            }
            "textDocument/documentSymbol" => {
                handle_document_symbols(&mut writer, id, &msg, &store);
            }
            "textDocument/formatting" => {
                handle_formatting(&mut writer, id, &msg, &store);
            }
            "textDocument/onTypeFormatting" => {
                handle_on_type_formatting(&mut writer, id, &msg, &store);
            }
            "textDocument/rangeFormatting" => {
                handle_range_formatting(&mut writer, id, &msg, &store);
            }
            "textDocument/foldingRange" => {
                handle_folding_ranges(&mut writer, id, &msg, &store);
            }
            "textDocument/documentHighlight" => {
                handle_document_highlight(&mut writer, id, &msg, &store);
            }
            "textDocument/selectionRange" => {
                handle_selection_ranges(&mut writer, id, &msg, &store);
            }
            "textDocument/codeAction" => {
                handle_code_actions(&mut writer, id, &msg, &store);
            }
            "textDocument/inlayHint" => {
                handle_inlay_hints(&mut writer, id, &msg, &store, &refinement_engine);
            }
            "textDocument/inlayHint/resolve" => {
                handle_inlay_hint_resolve(&mut writer, id, &msg);
            }
            "textDocument/definition" => {
                handle_definition(&mut writer, id, &msg, &store);
            }
            "textDocument/typeDefinition" => {
                handle_type_definition(&mut writer, id, &msg, &store, &refinement_engine);
            }
            "textDocument/implementation" => {
                handle_implementation(&mut writer, id, &msg, &store);
            }
            "textDocument/linkedEditingRange" => {
                handle_linked_editing(&mut writer, id, &msg, &store);
            }
            "textDocument/moniker" => {
                handle_moniker(&mut writer, id, &msg, &store);
            }
            "completionItem/resolve" => {
                handle_completion_resolve(&mut writer, id, &msg);
            }
            "workspace/executeCommand" => {
                handle_execute_command(&mut writer, id, &msg);
            }
            "textDocument/documentLink" => {
                handle_document_link(&mut writer, id, &msg, &store);
            }
            "textDocument/prepareCallHierarchy" => {
                handle_prepare_call_hierarchy(&mut writer, id, &msg, &store);
            }
            "callHierarchy/incomingCalls" => {
                handle_call_hierarchy_incoming(&mut writer, id, &msg);
            }
            "callHierarchy/outgoingCalls" => {
                handle_call_hierarchy_outgoing(&mut writer, id, &msg);
            }
            "textDocument/references" => {
                handle_references(&mut writer, id, &msg, &store);
            }
            "textDocument/prepareRename" => {
                handle_prepare_rename(&mut writer, id, &msg, &store);
            }
            "textDocument/rename" => {
                handle_rename(&mut writer, id, &msg, &store);
            }
            "textDocument/didOpen" => {
                if let Some(p) = &msg.params {
                    if let (Some(uri), Some(text)) = (
                        p.get("textDocument")
                            .and_then(|td| td.get("uri"))
                            .and_then(|u| u.as_str()),
                        p.get("textDocument")
                            .and_then(|td| td.get("text"))
                            .and_then(|t| t.as_str()),
                    ) {
                        store.open(uri, text);
                        let text = store.get(uri).map(|s| s.to_string()).unwrap_or_default();
                        handle_publish_diagnostics(
                            &mut writer,
                            uri,
                            &text,
                            &refinement_engine,
                        );
                        send_log_message(&mut writer, 4, &format!("opened: {}", uri));
                    }
                }
            }
            "textDocument/didChange" => {
                if let Some(p) = &msg.params {
                    if let (Some(uri), Some(changes)) = (
                        p.get("textDocument")
                            .and_then(|td| td.get("uri"))
                            .and_then(|u| u.as_str()),
                        p.get("contentChanges").and_then(|c| c.as_array()),
                    ) {
                        for change in changes {
                            if let Some(text) = change.get("text").and_then(|t| t.as_str()) {
                                if let Some(range) = change.get("range") {
                                    let current =
                                        store.get(uri).map(|s| s.to_string()).unwrap_or_default();
                                    let start = position_from(range.get("start"), &current);
                                    let end = position_from(range.get("end"), &current);
                                    let _ = store.apply_incremental(
                                        uri,
                                        TextContentChange {
                                            start,
                                            end,
                                            text: text.to_string(),
                                        },
                                    );
                                } else {
                                    store.update_full(uri, text);
                                }
                            }
                        }
                        if let Some(current) = store.get(uri) {
                            handle_publish_diagnostics(
                                &mut writer,
                                uri,
                                current,
                                &refinement_engine,
                            );
                        }
                        send_log_message(&mut writer, 4, &format!("changed: {}", uri));
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = msg
                    .params
                    .as_ref()
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|td| td.get("uri"))
                    .and_then(|u| u.as_str())
                {
                    store.close(uri);
                    send_log_message(&mut writer, 4, &format!("closed: {}", uri));
                }
            }
            "textDocument/willSave" => {}
            "textDocument/didSave" => {
                if let Some(p) = &msg.params {
                    if let Some(uri) = p
                        .get("textDocument")
                        .and_then(|td| td.get("uri"))
                        .and_then(|u| u.as_str())
                    {
                        let text = p
                            .get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                uri.strip_prefix("file://")
                                    .and_then(|path| std::fs::read_to_string(path).ok())
                            })
                            .unwrap_or_default();
                        store.update_full(uri, &text);
                        let text = store.get(uri).map(|s| s.to_string()).unwrap_or_default();
                        handle_publish_diagnostics(
                            &mut writer,
                            uri,
                            &text,
                            &refinement_engine,
                        );
                    }
                }
            }
            "$/cancelRequest" => {
                if let Some(p) = &msg.params {
                    if let Some(cancel_id) = p.get("id") {
                        cancelled.insert(cancel_id.clone());
                    }
                }
            }
            "workspace/didChangeConfiguration" => {
                send_log_message(&mut writer, 4, "configuration changed");
            }
            "workspace/symbol" => {
                handle_workspace_symbols(&mut writer, id, &msg);
            }
            "shutdown" => {
                handle_shutdown(&mut writer, id);
                send_log_message(&mut writer, 3, "handl-lsp shutting down");
            }
            "exit" => {
                break;
            }
            _ => {
                if let Some(ref id) = msg.id {
                    send_log_message(&mut writer, 2, &format!("unknown method: {}", msg.method));
                    handle_unknown(&mut writer, id.clone());
                }
            }
        }

        if let Some(ref req_id) = msg.id {
            cancelled.remove(req_id);
        }
    }
}

fn is_cancelled(id: &serde_json::Value, cancelled: &HashSet<serde_json::Value>) -> bool {
    cancelled.contains(id)
}

fn position_from(pos: Option<&serde_json::Value>, text: &str) -> usize {
    pos.and_then(|p| {
        let line = p.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
        let character = p.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
        Some(document::position_to_offset(text, line, character))
    })
    .unwrap_or(0)
}
