use std::io::Write;

use serde_json::{json, Value};
use crate::protocol::send_response;
use crate::protocol::send_error;
use crate::protocol::send_log_message;
pub fn handle_shutdown(writer: &mut impl Write, id: Value) {
    send_response(writer, id, json!(null));
}

pub fn handle_unknown(writer: &mut impl Write, id: Value) {
    send_error(writer, id, -32601, "method not found");
}

#[allow(dead_code)]
pub fn handle_log(writer: &mut impl Write, level: u32, message: &str) {
    send_log_message(writer, level, message);
}

