use std::io::Write;

use crate::execute_command;
use crate::protocol::Message;
use crate::protocol::send_response;
use serde_json::Value;
pub fn handle_execute_command(writer: &mut impl Write, id: Value, msg: &Message) {
    let command = msg
        .params
        .as_ref()
        .and_then(|p| p.get("command"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let arguments: Vec<Value> = msg
        .params
        .as_ref()
        .and_then(|p| p.get("arguments"))
        .and_then(|a| a.as_array())
        .map(|a| a.clone())
        .unwrap_or_default();
    let result = execute_command::execute_command(command, &arguments);
    send_response(writer, id, result);
}
