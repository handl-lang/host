use serde_json::{Value, json};

use handl_kit_canonical::{LexKind, Kit};

use crate::diagnostics::offset_to_position;

pub fn hover_at(source: &str, offset: usize) -> Option<Value> {
    let tokens = Kit::new().lex(source).ok()?;
    let token = tokens
        .iter()
        .find(|t| t.span.start <= offset && offset < t.span.end && !matches!(t.kind, LexKind::Eof))?;

    let markdown = match &token.kind {
        LexKind::Number(n) => format!("`number`\n\nValue: `{n}`"),
        LexKind::String(s) => format!("`string`\n\nValue: `{}`", escape_string(s)),
        LexKind::Bool(b) => format!("`boolean`\n\nValue: `{b}`"),
        LexKind::Null => "`null`".to_string(),
        LexKind::Identifier(name) => format!("`identifier`\n\nName: `{name}`"),
        LexKind::Keyword(_) => format!("`keyword`\n\n`{}`", token.kind),
        LexKind::Operator(_) => format!("`operator`\n\n`{}`", token.kind),
        LexKind::Delimiter(_) => format!("`punctuation`\n\n`{}`", token.kind),
        LexKind::Comment(_) | LexKind::DocComment(_) => "`comment`".to_string(),
        LexKind::Template(_) => "`template string`".to_string(),
        LexKind::Eof => return None,
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
