use serde_json::{Value, json};

use handl_kit_canonical::{CanonicalKind, Kit};

use crate::diagnostics::offset_to_position;

pub fn hover_at(source: &str, offset: usize) -> Option<Value> {
    let tokens = Kit::new().lex(source).ok()?;
    let token = tokens
        .iter()
        .find(|t| t.span.start <= offset && offset < t.span.end && !matches!(t.kind, CanonicalKind::Eof))?;

    let markdown = match &token.kind {
        CanonicalKind::Number(n) => format!("`number`\n\nValue: `{n}`"),
        CanonicalKind::String(s) => format!("`string`\n\nValue: `{}`", escape_string(s)),
        CanonicalKind::Bool(b) => format!("`boolean`\n\nValue: `{b}`"),
        CanonicalKind::Null => "`null`".to_string(),
        CanonicalKind::Identifier(name) => format!("`identifier`\n\nName: `{name}`"),
        CanonicalKind::Keyword(_) => format!("`keyword`\n\n`{}`", token.kind),
        CanonicalKind::Operator(_) => format!("`operator`\n\n`{}`", token.kind),
        CanonicalKind::Delimiter(_) => format!("`punctuation`\n\n`{}`", token.kind),
        CanonicalKind::Comment(_) | CanonicalKind::DocComment(_) => "`comment`".to_string(),
        CanonicalKind::Template(_) => "`template string`".to_string(),
        CanonicalKind::Eof => return None,
    };

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

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
