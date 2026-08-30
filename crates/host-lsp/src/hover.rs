use serde_json::{Value, json};

use handl_kit_canonical::{
    is_bool_kind, is_comment_kind, is_delimiter_kind, is_doc_comment_kind, is_identifier_kind,
    is_keyword_kind, is_null_kind, is_number_kind, is_operator_kind, is_string_kind,
    is_template_kind, Kit, SpannedToken,
};

use crate::diagnostics::offset_to_position;

pub fn hover_at(source: &str, offset: usize) -> Option<Value> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let token = tokens
        .iter()
        .find(|t| t.span.start <= offset && offset < t.span.end)?;

    let markdown = classify_token(token)?;

    let (start_line, start_char) = offset_to_position(source, token.span.start);
    let (end_line, end_char) = offset_to_position(source, token.span.end);

    Some(json!({
        "contents": { "kind": "markdown", "value": markdown },
        "range": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": end_line, "character": end_char },
        }
    }))
}

fn classify_token(token: &SpannedToken) -> Option<String> {
    if is_number_kind(&token.kind) {
        Some(format!("`number`\n\nValue: `{}`", token.text))
    } else if is_string_kind(&token.kind) {
        let s = &token.text;
        let inner = &s[1..s.len().saturating_sub(1)];
        Some(format!("`string`\n\nValue: `{}`", escape_string(inner)))
    } else if is_bool_kind(&token.kind) {
        Some(format!("`boolean`\n\nValue: `{}`", token.text))
    } else if is_null_kind(&token.kind) {
        Some("`null`".to_string())
    } else if is_identifier_kind(&token.kind) {
        Some(format!("`identifier`\n\nName: `{}`", token.text))
    } else if is_keyword_kind(&token.kind) {
        Some(format!("`keyword`\n\n`{}`", token.text))
    } else if is_operator_kind(&token.kind) {
        Some(format!("`operator`\n\n`{}`", token.text))
    } else if is_delimiter_kind(&token.kind) {
        Some(format!("`punctuation`\n\n`{}`", token.text))
    } else if is_doc_comment_kind(&token.kind) {
        Some("`comment`".to_string())
    } else if is_comment_kind(&token.kind) {
        Some("`comment`".to_string())
    } else if is_template_kind(&token.kind) {
        Some("`template string`".to_string())
    } else {
        None
    }
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
