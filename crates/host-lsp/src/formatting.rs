//! Document formatting for the `textDocument/formatting` LSP request.
//!
//! Auto-indents `.hnl` arrow files using a line-based formatter:
//! - 2-space indentation, tracking nesting depth via `[]`, `{}`, `()`.
//! - Indent increases after opening `{`, `(`, `[` and decreases before a
//!   line that starts with a closing `}`, `)`, `]`.
//! - Colons (`:`) in consecutive object key-value pairs within the same
//!   block are aligned.
//! - Trailing whitespace is stripped from every line.
//! - The file is ensured to end with exactly one newline.
//! - Comment content is never reformatted (only its indentation is fixed).
//! - Long lines are never broken.
//! - Only lines that actually changed produce `TextEdit`s.

use serde_json::{Value, json};

/// A single LSP text edit: replace the range `[start, end)` with `new_text`.
///
/// Line/character positions are 0-based; characters are counted as Unicode
/// scalar values (matching the rest of the LSP's `offset_to_position`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start_line: usize,
    pub start_char: usize,
    pub end_line: usize,
    pub end_char: usize,
    pub new_text: String,
}

/// Format `source`, returning edits only for lines that changed.
pub fn format_document(source: &str) -> Vec<TextEdit> {
    if source.is_empty() {
        return Vec::new();
    }

    // Strip all trailing newlines; we'll re-add exactly one at the end.
    let content_part = source.trim_end_matches('\n');
    // `split('\n')` on "" yields `[""]`, which is a single (blank) line.
    let real_lines: Vec<&str> = content_part.split('\n').collect();

    // Count how many trailing newlines the original source had.
    let trailing_newline_count = source.chars().rev().take_while(|&c| c == '\n').count();

    // --- First pass: compute indent depth and trimmed content per line. ---
    let mut depth: i32 = 0;
    let mut in_block_comment = false;

    struct LineInfo {
        indent: usize,
        content: String, // trimmed (no leading/trailing whitespace)
        is_blank: bool,
        is_comment: bool,
    }

    let mut infos: Vec<LineInfo> = Vec::with_capacity(real_lines.len());
    for line in &real_lines {
        let stripped = line.trim_end();
        let trimmed = stripped.trim_start();
        let is_blank = trimmed.is_empty();
        let is_comment = in_block_comment || trimmed.starts_with("//");

        let indent = if is_blank {
            0
        } else if starts_with_closing(trimmed) {
            (depth - 1).max(0) as usize
        } else {
            depth as usize
        };

        // Update bracket depth and block-comment state from this line.
        let (delta, still_in_block) = scan_brackets(stripped, in_block_comment);
        in_block_comment = still_in_block;
        depth = (depth + delta).max(0);

        infos.push(LineInfo {
            indent,
            content: trimmed.to_string(),
            is_blank,
            is_comment,
        });
    }

    // --- Second pass: produce formatted text, aligning colon groups. ---
    let mut formatted: Vec<String> = vec![String::new(); infos.len()];
    let mut i = 0;
    while i < infos.len() {
        let info = &infos[i];
        if info.is_blank || info.is_comment || !is_keyval(&info.content) {
            formatted[i] = indent_str(info.indent) + &info.content;
            i += 1;
            continue;
        }
        // Gather a run of consecutive key-value lines at the same indent.
        let group_indent = info.indent;
        let mut j = i;
        while j < infos.len()
            && !infos[j].is_blank
            && !infos[j].is_comment
            && infos[j].indent == group_indent
            && is_keyval(&infos[j].content)
        {
            j += 1;
        }
        let group: Vec<usize> = (i..j).collect();
        if group.len() >= 2 {
            let max_key = group
                .iter()
                .map(|&k| key_of(&infos[k].content).chars().count())
                .max()
                .unwrap_or(0);
            for &k in &group {
                let (key, value) = split_keyval(&infos[k].content);
                let padding = max_key.saturating_sub(key.chars().count());
                formatted[k] = format!(
                    "{}{}{}: {}",
                    indent_str(group_indent),
                    key,
                    " ".repeat(padding),
                    value
                );
            }
        } else {
            // Singleton key-value line: leave content as-is (just indent).
            formatted[i] = indent_str(info.indent) + &info.content;
        }
        i = j;
    }

    // --- Third pass: diff against originals and emit edits. ---
    let mut edits: Vec<TextEdit> = Vec::new();
    let last_idx = real_lines.len().saturating_sub(1);

    for (idx, original) in real_lines.iter().enumerate() {
        if formatted[idx] != *original {
            let orig_char_len = original.chars().count();
            // If this is the last line and we need to ADD a newline (none
            // present), fold the newline into this edit's new_text so the
            // edits stay non-overlapping.
            let is_last = idx == last_idx;
            let needs_added_newline = is_last && trailing_newline_count == 0;
            let new_text = if needs_added_newline {
                format!("{}\n", formatted[idx])
            } else {
                formatted[idx].clone()
            };
            edits.push(TextEdit {
                start_line: idx,
                start_char: 0,
                end_line: idx,
                end_char: orig_char_len,
                new_text,
            });
        }
    }

    // --- Final-newline adjustments that aren't folded into a line edit. ---
    if trailing_newline_count == 0 {
        // No content line changed on the last line but we still need a
        // trailing newline: insert one at the end of the last line.
        if !edits.iter().any(|e| e.start_line == last_idx) {
            let orig_char_len = real_lines[last_idx].chars().count();
            edits.push(TextEdit {
                start_line: last_idx,
                start_char: orig_char_len,
                end_line: last_idx,
                end_char: orig_char_len,
                new_text: "\n".to_string(),
            });
        }
    } else if trailing_newline_count >= 2 {
        // Remove the extra trailing newlines (keep exactly one). The first
        // newline terminates the last content line; the rest live on the
        // lines after it.
        edits.push(TextEdit {
            start_line: last_idx + 1,
            start_char: 0,
            end_line: last_idx + trailing_newline_count,
            end_char: 0,
            new_text: String::new(),
        });
    }

    edits
}

/// Wrap the formatting edits in an LSP `textDocument/formatting` response
/// (a JSON array of `{ range, newText }` objects).
pub fn format_document_json(source: &str) -> Value {
    let edits = format_document(source);
    let arr: Vec<Value> = edits
        .iter()
        .map(|e| {
            json!({
                "range": {
                    "start": { "line": e.start_line, "character": e.start_char },
                    "end": { "line": e.end_line, "character": e.end_char },
                },
                "newText": e.new_text,
            })
        })
        .collect();
    json!(arr)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Two spaces per indent level.
fn indent_str(depth: usize) -> String {
    "  ".repeat(depth)
}

/// True if the (trimmed) line content begins with a closing bracket.
fn starts_with_closing(s: &str) -> bool {
    matches!(s.chars().next(), Some('}') | Some(')') | Some(']'))
}

/// Scan `s` for bracket nesting deltas, skipping over line/block comments
/// and string literals. Returns `(net_delta, still_in_block_comment)`.
fn scan_brackets(s: &str, mut in_block: bool) -> (i32, bool) {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut delta: i32 = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_block {
            if c == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_string {
            if c == '\\' && i + 1 < chars.len() {
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            break; // line comment: rest of line ignored
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            in_block = true;
            i += 2;
            continue;
        }
        if c == '"' {
            in_string = true;
            i += 1;
            continue;
        }
        match c {
            '{' | '(' | '[' => delta += 1,
            '}' | ')' | ']' => delta -= 1,
            _ => {}
        }
        i += 1;
    }
    (delta, in_block)
}

/// True if `content` (already trimmed) looks like an object key-value pair:
/// `identifier: value`, where the key is a simple identifier and a value
/// follows the colon.
fn is_keyval(content: &str) -> bool {
    let colon_idx = match content.find(':') {
        Some(idx) => idx,
        None => return false,
    };
    let key = content[..colon_idx].trim();
    if key.is_empty() {
        return false;
    }
    let mut chars = key.chars();
    let first = chars.next().unwrap();
    if !(first.is_alphabetic() || first == '_') {
        return false;
    }
    if !chars.all(|c| c.is_alphanumeric() || c == '_') {
        return false;
    }
    // There must be a non-empty value after the colon.
    !content[colon_idx + 1..].trim().is_empty()
}

/// Extract the key portion (before the first colon) of a key-value line.
fn key_of(content: &str) -> String {
    content
        .find(':')
        .map(|idx| content[..idx].trim().to_string())
        .unwrap_or_default()
}

/// Split a key-value line into `(key, value)`, both trimmed.
fn split_keyval(content: &str) -> (String, String) {
    let colon_idx = content.find(':').unwrap();
    (
        content[..colon_idx].trim().to_string(),
        content[colon_idx + 1..].trim().to_string(),
    )
}
