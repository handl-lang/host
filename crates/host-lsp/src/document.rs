use std::collections::HashMap;

use handl_host_tooling::{BasicIncrementalSync, IncrementalSyncPlugin, TextContentChange};
use serde_json::Value;

pub struct DocumentStore {
    docs: HashMap<String, String>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    pub fn open(&mut self, uri: &str, text: &str) {
        self.docs.insert(uri.to_string(), text.to_string());
    }

    pub fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }

    pub fn get(&self, uri: &str) -> Option<&str> {
        self.docs.get(uri).map(|s| s.as_str())
    }

    pub fn update_full(&mut self, uri: &str, text: &str) {
        self.docs.insert(uri.to_string(), text.to_string());
    }

    pub fn apply_incremental(
        &mut self,
        uri: &str,
        change: TextContentChange,
    ) -> Option<&str> {
        let sync = BasicIncrementalSync::new();
        let current = self.docs.get(uri)?.clone();
        let resp = sync
            .run(&handl_host_tooling::IncrementalSyncRequest {
                source: current,
                change,
            })
            .ok()?;
        self.docs.insert(uri.to_string(), resp.0);
        self.docs.get(uri).map(|s| s.as_str())
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

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

pub fn resolve_document_text(store: &DocumentStore, params: &Option<Value>) -> String {
    if let Some(uri) = params
        .as_ref()
        .and_then(|p| p.get("textDocument"))
        .and_then(|td| td.get("uri"))
        .and_then(|uri| uri.as_str())
    {
        if let Some(text) = store.get(uri) {
            return text.to_string();
        }
    }
    get_document_text(params)
}

pub fn position_to_offset(text: &str, line: u32, character: u32) -> usize {
    let mut line_start = 0;
    let mut current_line: u32 = 0;
    for (i, ch) in text.char_indices() {
        if current_line == line {
            break;
        }
        if ch == '\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    if current_line < line {
        return text.len();
    }
    let mut current_char: u32 = 0;
    for (i, ch) in text[line_start..].char_indices() {
        if ch == '\n' {
            return line_start + i;
        }
        if current_char == character {
            return line_start + i;
        }
        current_char += 1;
    }
    text.len()
}
