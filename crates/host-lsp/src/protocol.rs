use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Write;

#[derive(Debug, Deserialize)]
pub struct Message {
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub id: Option<Value>,
}

pub fn send_response(writer: &mut impl Write, id: Value, result: Value) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    write_message(writer, &response);
}

pub fn send_error(writer: &mut impl Write, id: Value, code: i32, message: &str) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    });
    write_message(writer, &response);
}

pub fn send_notification(writer: &mut impl Write, method: &str, params: Value) {
    let notification = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    write_message(writer, &notification);
}

fn write_message(writer: &mut impl Write, body: &Value) {
    let serialized = serde_json::to_string(body).unwrap();
    let _ = write!(
        writer,
        "Content-Length: {}\r\n\r\n{}",
        serialized.len(),
        serialized
    );
    let _ = writer.flush();
}
