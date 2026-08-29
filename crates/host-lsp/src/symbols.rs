use serde_json::{Value, json};

use handl_kit_canonical::{CanonicalKind, CanonicalOp, Kit};
use handl_plugin_lexer::{CommentTokenKind, Span};

use crate::diagnostics::offset_to_position;

pub fn identifier_at_offset(source: &str, offset: usize) -> Option<(String, Span)> {
    let tokens = Kit::new().lex(source).ok()?;
    for t in &tokens {
        if let CanonicalKind::Identifier(ref name) = t.kind {
            if t.span.start <= offset && offset < t.span.end {
                return Some((name.clone(), t.span));
            }
        }
    }
    None
}

pub fn find_definition(source: &str, name: &str) -> Option<Span> {
    let tokens = Kit::new().lex(source).ok()?;
    let mut i = 0;
    while i < tokens.len() {
        if let CanonicalKind::Identifier(ref n) = tokens[i].kind {
            if n == name {
                // find next non-comment token
                let mut j = i + 1;
                while j < tokens.len() && tokens[j].kind.is_comment() {
                    j += 1;
                }
                if j < tokens.len() {
                    if let CanonicalKind::Operator(CanonicalOp::Equal) = tokens[j].kind {
                        return Some(tokens[i].span);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

pub fn find_references(source: &str, name: &str) -> Vec<Span> {
    let tokens = match Kit::new().lex(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    tokens
        .iter()
        .filter_map(|t| {
            if let CanonicalKind::Identifier(ref n) = t.kind {
                if n == name {
                    return Some(t.span);
                }
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
