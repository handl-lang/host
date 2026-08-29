use serde_json::Value;

pub fn get_document_text(params: &Option<Value>) -> String {
    params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|uri| uri.as_str())
        .and_then(|uri| uri.strip_prefix("file://"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}
