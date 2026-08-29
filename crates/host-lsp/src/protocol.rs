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
    let serialized = serde_json::to_string(&response).unwrap();
    let _ = writeln!(writer, "{serialized}");
    let _ = writer.flush();
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
    let serialized = serde_json::to_string(&response).unwrap();
    let _ = writeln!(writer, "{serialized}");
    let _ = writer.flush();
}
