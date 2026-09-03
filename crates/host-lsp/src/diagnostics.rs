use serde_json::{Value, json};

use handl_language::LangError;
use handl_plugin_ext_refinement::parse_arrow_file;

use crate::kit_resolver::kit_for_file;
use crate::refinement::RefinementDiagnostic;

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
    let kit = match kit_for_file(None) {
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
        Ok(_) => {
            // Strip comments from the source before parsing — the Pratt parser
            // doesn't skip comment tokens, so we replace them with whitespace
            // to preserve byte positions for error reporting.
            let stripped = strip_comments(source);
            match kit.parse_module(&stripped) {
                Err(e) => {
                    // Suppress "unexpected token: token.comment" errors —
                    // these are caused by comments the parser doesn't handle,
                    // not actual syntax errors in the source.
                    let msg = e.to_string();
                    if msg.contains("token.comment") {
                        return Vec::new();
                    }
                    vec![json!({
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 0 },
                        },
                        "severity": 1,
                        "message": msg,
                    })]
                }
                Ok(_) => Vec::new(),
            }
        }
    }
}

/// Replace comment text with whitespace (preserving positions) so the parser
/// doesn't choke on `token.comment` tokens.
fn strip_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.char_indices().peekable();
    let bytes = source.as_bytes();

    while let Some((i, ch)) = chars.next() {
        // Line comment: -- ... end of line
        if ch == '-' && bytes.get(i + 1) == Some(&b'-') {
            // Replace the rest of the line with spaces
            result.push(' ');
            result.push(' ');
            // Skip to end of line
            for (_, c) in chars.by_ref() {
                if c == '\n' {
                    result.push('\n');
                    break;
                }
                result.push(' ');
            }
        }
        // Block comment: /* ... */
        else if ch == '/' && bytes.get(i + 1) == Some(&b'*') {
            result.push(' ');
            result.push(' ');
            chars.next(); // skip the '*'
            while let Some((_, c)) = chars.next() {
                if c == '*' && chars.peek().map(|(_, c)| *c) == Some('/') {
                    result.push(' ');
                    result.push(' ');
                    chars.next(); // skip the '/'
                    break;
                }
                if c == '\n' {
                    result.push('\n');
                } else {
                    result.push(' ');
                }
            }
        }
        else {
            result.push(ch);
        }
    }
    result
}

/// Produce parse-error diagnostics for a source document.
///
/// This runs the canonical lexer (`Kit::lex_with_spans`) and the arrow-file
/// parser (`parse_arrow_file`) over the in-memory source so that real syntax
/// errors are surfaced in the editor as the user types. Lexing failures
/// produce a single diagnostic pinned to line 0 (the lexer reports a byte
/// offset, but a stable line is enough for a squiggle). Parse failures
/// produce a single diagnostic; the message is inspected for a `line N`
/// pattern to position the diagnostic when possible, otherwise line 0.
///
/// Returns an empty vector when the source lexes and parses cleanly.
pub fn parse_diagnostics(source: &str) -> Vec<RefinementDiagnostic> {
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(e) => {
            return vec![RefinementDiagnostic {
                line: 0,
                character: 0,
                message: e.to_string(),
            }];
        }
    };

    match kit.lex_with_spans(source) {
        Err(e) => {
            // The lexer reports a byte offset; pin the diagnostic to line 0
            // per the LSP diagnostics contract for lex failures.
            vec![RefinementDiagnostic {
                line: 0,
                character: 0,
                message: e.to_string(),
            }]
        }
        Ok(_) => match parse_arrow_file(source) {
            Ok(_) => Vec::new(),
            Err(msg) => {
                let line = extract_line_from_message(&msg).unwrap_or(0);
                vec![RefinementDiagnostic {
                    line,
                    character: 0,
                    message: msg,
                }]
            }
        },
    }
}

/// Best-effort extraction of a line number from a parser error message.
///
/// Looks for patterns like `line 12` or `line:12` and returns the first
/// match. Returns `None` when no line number can be found.
fn extract_line_from_message(msg: &str) -> Option<u32> {
    let mut tokens = msg.split_whitespace();
    while let Some(tok) = tokens.next() {
        let stripped = tok.trim_end_matches(':');
        if stripped.eq_ignore_ascii_case("line") {
            if let Some(num) = tokens.next() {
                let cleaned = num.trim_end_matches(':');
                if let Ok(n) = cleaned.parse::<u32>() {
                    return Some(n);
                }
            }
        }
        // Also handle "line:12" without surrounding whitespace.
        if let Some(rest) = tok.strip_prefix("line:") {
            if let Ok(n) = rest.parse::<u32>() {
                return Some(n);
            }
        }
    }
    None
}
