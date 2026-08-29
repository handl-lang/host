mod document;
mod handlers;
mod protocol;

#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod handlers_tests;
#[cfg(test)]
mod protocol_tests;

use std::io::{self, BufRead};

use handl_host_tooling::{SemanticTokensPlugin, canonical_semantic_tokens};

use handlers::{handle_initialize, handle_semantic_tokens_full, handle_shutdown, handle_unknown};
use protocol::Message;

fn main() {
    let tokens_provider = canonical_semantic_tokens();
    let legend = tokens_provider.legend();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.is_empty() {
            continue;
        }

        let msg: Message = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };

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
