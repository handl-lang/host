mod document;
mod handlers;
mod protocol;

#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod handlers_tests;
#[cfg(test)]
mod protocol_tests;

use std::io::{self, BufRead, Read};

use handl_host_tooling::{SemanticTokensPlugin, canonical_semantic_tokens};

use handlers::{handle_initialize, handle_semantic_tokens_full, handle_shutdown, handle_unknown};
use protocol::Message;

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

    while let Some(msg) = read_message(&mut reader) {
        let id = msg.id.clone().unwrap_or(serde_json::Value::Null);

        match msg.method.as_str() {
            "initialize" => {
                handle_initialize(&mut writer, id, &legend);
            }
            "initialized" => {}
            "textDocument/semanticTokens/full" => {
                handle_semantic_tokens_full(&mut writer, id, &msg, &tokens_provider);
            }
            "shutdown" => {
                handle_shutdown(&mut writer, id);
            }
            "exit" => {
                break;
            }
            _ => {
                if let Some(id) = msg.id {
                    handle_unknown(&mut writer, id);
                }
            }
        }
    }
}
