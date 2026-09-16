use std::io::Write;

use serde_json::{json, Value};
use handl_host_tooling::ToolingHost;
use crate::protocol::Message;
use crate::protocol::send_response;
use crate::workspace_symbols;
pub fn handle_workspace_symbols(
    writer: &mut impl Write,
    id: Value,
    msg: &Message,
    host: &ToolingHost,
) {
    let query = msg
        .params
        .as_ref()
        .and_then(|p| p.get("query"))
        .and_then(|q| q.as_str())
        .unwrap_or("");
    // Try to find workspace root from the first open document.
    let root = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    let symbols = workspace_symbols::workspace_symbols(query, root.as_deref(), host);
    send_response(writer, id, json!(symbols));
}

