use std::io::Write;

use super::position_offset_from_msg;
use crate::completion_resolve;
use crate::document::{DocumentStore, resolve_document_text};
use crate::protocol::Message;
use crate::protocol::send_error;
use crate::protocol::send_response;
use handl_host_tooling::ToolingHost;
use serde_json::{Value, json};
pub fn handle_completion_resolve(writer: &mut impl Write, id: Value, msg: &Message) {
    let item = msg
        .params
        .as_ref()
        .map(|p| p.clone())
        .unwrap_or(json!(null));
    let resolved = completion_resolve::resolve_completion(item);
    send_response(writer, id, resolved);
}

pub fn handle_completion(
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
    match host.complete(&source, offset, file_path) {
        Ok(items) => {
            let result: Vec<Value> = items
                .into_iter()
                .map(|item| {
                    let mut map = serde_json::Map::new();
                    map.insert("label".to_string(), json!(item.label));
                    map.insert("kind".to_string(), json!(item.kind.unwrap_or(6)));
                    if let Some(detail) = item.detail {
                        map.insert("detail".to_string(), json!(detail));
                    }
                    if let Some(text) = item.insert_text {
                        map.insert("insertText".to_string(), json!(text));
                    }
                    if let Some(fmt) = item.insert_text_format {
                        map.insert("insertTextFormat".to_string(), json!(fmt));
                    }
                    if let Some(edit) = item.text_edit {
                        map.insert("textEdit".to_string(), json!({
                            "range": {
                                "start": { "line": edit.range.start.line, "character": edit.range.start.character },
                                "end": { "line": edit.range.end.line, "character": edit.range.end.character },
                            },
                            "newText": edit.new_text,
                        }));
                    }
                    Value::Object(map)
                })
                .collect();
            send_response(writer, id, Value::Array(result));
        }
        Err(e) => {
            let msg = e.to_string();
            send_error(writer, id, -32603, &msg);
        }
    }
}
