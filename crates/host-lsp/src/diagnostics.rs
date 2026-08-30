use serde_json::{Value, json};

use handl_kit_canonical::Kit;
use handl_language::LangError;

pub fn offset_to_position(text: &str, offset: usize) -> (u32, u32) {
    let mut offset = offset.min(text.len());
    if !text.is_char_boundary(offset) {
        while offset > 0 && !text.is_char_boundary(offset) {
            offset -= 1;
        }
    }
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    for (i, ch) in text.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn span_to_range(text: &str, start: usize, end: usize) -> Value {
    let (start_line, start_char) = offset_to_position(text, start);
    let (end_line, end_char) = offset_to_position(text, end);
    json!({
        "start": { "line": start_line, "character": start_char },
        "end": { "line": end_line, "character": end_char },
    })
}

pub fn collect_diagnostics(source: &str) -> Vec<Value> {
    let kit = match Kit::new() {
        Ok(k) => k,
        Err(e) => {
            return vec![json!({
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 },
                },
                "severity": 1,
                "message": e.to_string(),
            })];
        }
    };
    match kit.lex(source) {
        Err(e) => {
            let (start, end) = match &e {
                LangError::LexError { position, .. } => (*position, *position + 1),
                _ => (0, 0),
            };
            vec![json!({
                "range": span_to_range(source, start, end),
                "severity": 1,
                "message": e.to_string(),
            })]
        }
        Ok(_) => match kit.parse_module(source) {
            Err(e) => vec![json!({
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 },
                },
                "severity": 1,
                "message": e.to_string(),
            })],
            Ok(_) => Vec::new(),
        },
    }
}
