use std::io::Write;

use super::position_offset_from_msg;
use crate::document::{DocumentStore, resolve_document_text};
use crate::protocol::{Message, send_error, send_response};
use handl_host_tooling::{ToolingHost, lsp::SignatureHelpRequest};
use serde_json::{Value, json};

pub fn handle_signature_help(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    store: &DocumentStore,
    host: &ToolingHost,
) {
    let source = resolve_document_text(store, &msg.params);
    let offset = position_offset_from_msg(&msg, &source);
    let file_path = msg
        .params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .and_then(|s| s.strip_prefix("file://"))
        .unwrap_or("");

    let request = SignatureHelpRequest {
        source,
        pos: offset,
        path: file_path.to_string(),
    };

    match host.signature_help(&request) {
        Ok(resp) if !resp.signatures.is_empty() => {
            let sig = &resp.signatures[0];
            let params_json: Vec<Value> = sig
                .parameters
                .iter()
                .map(|p| {
                    json!({
                        "label": p.label,
                        "documentation": p.documentation,
                    })
                })
                .collect();
            let result = json!({
                "signatures": [{
                    "label": sig.label,
                    "documentation": sig.documentation.as_deref().unwrap_or(""),
                    "parameters": params_json,
                }],
                "activeSignature": 0,
                "activeParameter": resp.active_parameter,
            });
            send_response(writer, id, result);
        }
        Ok(_) => send_response(writer, id, json!(null)),
        Err(e) => {
            let message = e.to_string();
            send_error(writer, id, -32603, &message);
        }
    }
}
