mod completion;
mod diagnostics;
mod document;
mod handlers;
mod hover;
mod protocol;
mod symbols;

#[cfg(test)]
mod cancellation_tests;
#[cfg(test)]
mod completion_tests;
#[cfg(test)]
mod diagnostics_tests;
#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod handlers_tests;
#[cfg(test)]
mod hover_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod symbols_tests;

use std::collections::HashSet;
use std::io::{self, BufRead};

use handl_host_tooling::{SemanticTokensPlugin, TextContentChange, canonical_semantic_tokens};

use document::DocumentStore;
use handlers::{
    handle_completion, handle_definition, handle_hover, handle_initialize,
    handle_publish_diagnostics, handle_references, handle_rename, handle_semantic_tokens_full,
    handle_semantic_tokens_range, handle_shutdown, handle_unknown,
};
use protocol::{Message, send_log_message};

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
                handle_hover(&mut writer, id, &msg, &store);
            }
            "textDocument/completion" => {
                handle_completion(&mut writer, id, &msg, &store);
            }
            "textDocument/definition" => {
                handle_definition(&mut writer, id, &msg, &store);
            }
            "textDocument/references" => {
                handle_references(&mut writer, id, &msg, &store);
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
                        handle_publish_diagnostics(&mut writer, uri, &text);
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
                            handle_publish_diagnostics(&mut writer, uri, current);
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
                        handle_publish_diagnostics(&mut writer, uri, &text);
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
