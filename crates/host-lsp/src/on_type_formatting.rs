//! On-type formatting for the `textDocument/onTypeFormatting` LSP request.
//!
//! As the user types in a `.hnl` arrow file, this module adjusts indentation:
//! - Closing brackets (`}`, `)`, `]`) are dedented to the indentation of the
//!   line that holds the matching opening bracket.
//! - A newline (`\n`) indents the new line based on the bracket-depth change of
//!   the previous line, plus one extra level when the previous line ends with a
//!   colon (`:`).
//!
//! Indentation is two spaces per nesting level, matching `crate::formatting`.

use serde_json::{Value, json};

use crate::diagnostics::offset_to_position;

/// Two spaces per indent level.
const INDENT_UNIT: &str = "  ";

/// Entry point for `textDocument/onTypeFormatting`.
///
/// `source` is the document text *after* the character `ch` was inserted at
/// `position` (0-based line, character). The returned `Vec<Value>` is a JSON
/// array of LSP `TextEdit` objects. When no adjustment is needed it is empty.
pub fn on_type_formatting(source: &str, position: (u32, u32), ch: &str) -> Vec<Value> {
    if source.is_empty() {
        return Vec::new();
    }
    match ch {
        "}" | ")" | "]" => {
            let close_ch = ch.chars().next().unwrap();
            on_close_bracket(source, position, close_ch)
        }
        "\n" => on_newline(source, position),
        // Opening braces and other characters need no on-type adjustment; the
        // full formatter (`crate::formatting`) handles them on explicit format.
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Closing brackets: `}`, `)`, `]`
// ---------------------------------------------------------------------------

fn on_close_bracket(source: &str, position: (u32, u32), close_ch: char) -> Vec<Value> {
    let (line, character) = position;
    let line_idx = line as usize;
    let lines: Vec<&str> = source.split('\n').collect();
    if line_idx >= lines.len() {
        return Vec::new();
    }
    let line_text = lines[line_idx];
    let chars: Vec<char> = line_text.chars().collect();
    let col = character as usize;
    // The character at the cursor must be the closing bracket that was typed.
    if col >= chars.len() || chars[col] != close_ch {
        return Vec::new();
    }
    // Only dedent when the closing bracket is the first non-whitespace on its
    // line (i.e. everything before it is whitespace). Inline closers are left
    // alone.
    let ws_before = chars[..col]
        .iter()
        .take_while(|c| c.is_whitespace())
        .count();
    if ws_before != col {
        return Vec::new();
    }
    let current_indent = col;

    // Locate the matching opening bracket by scanning backwards from the
    // typed closing bracket.
    let close_offset = position_to_offset(source, line, character);
    let open_offset = match find_matching_open(source, close_offset, close_ch) {
        Some(o) => o,
        None => return Vec::new(),
    };
    let open_line = offset_to_position(source, open_offset).0 as usize;
    if open_line >= lines.len() {
        return Vec::new();
    }
    let open_line_text = lines[open_line];
    let target_indent: String = open_line_text
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();

    // If the indentation is already correct, no edit is needed.
    if current_indent == target_indent.chars().count() {
        return Vec::new();
    }

    vec![text_edit(line, 0, current_indent as u32, &target_indent)]
}

/// Scan backwards from `close_offset` (which must point at `close_ch`) to find
/// the matching opening bracket, skipping over nested same-type pairs.
///
/// String literals and comments are not specially handled in this backward
/// scan; for well-formed arrow files this is sufficient because brackets
/// inside strings/comments are rare in practice and the full formatter is the
/// source of truth on explicit requests.
fn find_matching_open(source: &str, close_offset: usize, close_ch: char) -> Option<usize> {
    let open_ch = matching_open(close_ch)?;

    // Build a mapping from char index -> byte offset so we can return a byte
    // offset at the end.
    let mut char_to_byte: Vec<usize> = Vec::new();
    for (byte_idx, _) in source.char_indices() {
        char_to_byte.push(byte_idx);
    }
    let close_idx = char_to_byte.iter().position(|&b| b == close_offset)?;
    let chars: Vec<char> = source.chars().collect();

    let mut depth: i32 = 1;
    let mut i = close_idx;
    while i > 0 {
        i -= 1;
        let c = chars[i];
        if c == close_ch {
            depth += 1;
        } else if c == open_ch {
            depth -= 1;
            if depth == 0 {
                return Some(char_to_byte[i]);
            }
        }
    }
    None
}

fn matching_open(close: char) -> Option<char> {
    match close {
        '}' => Some('{'),
        ')' => Some('('),
        ']' => Some('['),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Newline: `\n`
// ---------------------------------------------------------------------------

fn on_newline(source: &str, position: (u32, u32)) -> Vec<Value> {
    let (line, _character) = position;
    let lines: Vec<&str> = source.split('\n').collect();
    let prev_idx = line as usize;
    // The previous line is the one the newline was appended to.
    if prev_idx >= lines.len() {
        return Vec::new();
    }
    let prev_line = lines[prev_idx];

    // Indentation of the previous line (assumed to be spaces).
    let prev_indent: String = prev_line
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    let prev_indent_len = prev_indent.chars().count() as i32;

    // Net bracket-depth change across the previous line.
    let delta = line_bracket_delta(prev_line);

    // One extra indent level when the previous line ends with a colon. Open
    // brackets are already accounted for by `delta`, so they do not need an
    // additional bump here (that would double-count).
    let trimmed = prev_line.trim_end();
    let extra = if trimmed.ends_with(':') { 2 } else { 0 };

    let new_len = (prev_indent_len + delta * 2 + extra).max(0) as usize;
    let new_indent = INDENT_UNIT.repeat(new_len / 2).chars().collect::<String>();
    // `new_len` is always even (prev_indent_len is even for space-indented
    // files, delta*2 and extra are even), so this is exact.

    // The new line is the one after the newline.
    let new_idx = prev_idx + 1;
    if new_idx >= lines.len() {
        return Vec::new();
    }
    let new_line = lines[new_idx];
    let existing_ws = new_line.chars().take_while(|c| c.is_whitespace()).count();

    if existing_ws == new_indent.chars().count() {
        return Vec::new();
    }

    vec![text_edit(
        new_idx as u32,
        0,
        existing_ws as u32,
        &new_indent,
    )]
}

/// Compute the net bracket nesting delta of a single line, skipping over line
/// comments, block comments, and string literals. Mirrors the logic in
/// `crate::formatting::scan_brackets` but returns only the delta (block-comment
/// state is not carried across lines here since we only inspect one line).
fn line_bracket_delta(s: &str) -> i32 {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut delta: i32 = 0;
    let mut in_string = false;
    let mut in_block = false;
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
            break;
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
    delta
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Convert a 0-based (line, character) position to a byte offset within
/// `text`. Characters are counted as Unicode scalar values.
fn position_to_offset(text: &str, line: u32, character: u32) -> usize {
    let mut line_start = 0usize;
    let mut current_line: u32 = 0;
    for (i, ch) in text.char_indices() {
        if current_line == line {
            line_start = i;
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

/// Build an LSP `TextEdit` JSON object replacing the range
/// `(line, start_char)..(line, end_char)` with `new_text`.
fn text_edit(line: u32, start_char: u32, end_char: u32, new_text: &str) -> Value {
    json!({
        "range": {
            "start": { "line": line, "character": start_char },
            "end": { "line": line, "character": end_char },
        },
        "newText": new_text,
    })
}
