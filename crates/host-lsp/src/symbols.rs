use serde_json::{Value, json};

use handl_kit_canonical::{
    SpannedToken, is_comment_kind, is_identifier_kind, is_operator_kind,
};
use handl_language::Span;

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;

pub fn identifier_at_offset(source: &str, offset: usize) -> Option<(String, Span)> {
    let kit = kit_for_file(None).ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    for t in &tokens {
        if is_identifier_kind(&t.kind) {
            if t.span.start <= offset && offset < t.span.end {
                return Some((t.text.clone(), t.span));
            }
        }
    }
    None
}

pub fn find_definition(source: &str, name: &str) -> Option<Span> {
    let kit = kit_for_file(None).ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let mut i = 0;
    while i < tokens.len() {
        if is_identifier_kind(&tokens[i].kind) && tokens[i].text == name {
            // Find next non-comment token.
            let mut j = i + 1;
            while j < tokens.len() && is_comment_kind(&tokens[j].kind) {
                j += 1;
            }
            if j < tokens.len() && is_operator_kind(&tokens[j].kind) && tokens[j].text == "=" {
                return Some(tokens[i].span);
            }
        }
        i += 1;
    }
    None
}

pub fn find_references(source: &str, name: &str) -> Vec<Span> {
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    tokens
        .iter()
        .filter_map(|t: &SpannedToken| {
            if is_identifier_kind(&t.kind) && t.text == name {
                return Some(t.span);
            }
            None
        })
        .collect()
}

pub fn span_to_range(source: &str, span: Span) -> Value {
    let (start_line, start_char) = offset_to_position(source, span.start);
    let (end_line, end_char) = offset_to_position(source, span.end);
    json!({
        "start": { "line": start_line, "character": start_char },
        "end": { "line": end_line, "character": end_char },
    })
}
