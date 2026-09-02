//! Range formatting for the `textDocument/rangeFormatting` LSP request.
//!
//! Formats only the lines within a selected `[start_line, end_line]` range
//! (inclusive), reusing the same line-based formatting rules as the
//! full-document formatter in [`crate::formatting`]:
//! - 2-space indentation, tracking nesting depth via `[]`, `{}`, `()`.
//! - Indent increases after opening brackets and decreases before a line
//!   that starts with a closing bracket.
//! - Colons (`:`) in consecutive object key-value pairs within the same
//!   block are aligned.
//! - Trailing whitespace is stripped from every line.
//! - Comment content is never reformatted (only its indentation is fixed).
//!
//! Because the range may begin inside a nested block, the bracket depth at
//! the start of `start_line` is computed by scanning the document from the
//! beginning up to that point.

use serde_json::{Value, json};

/// Format only the lines in `[start_line, end_line]` (inclusive) of `source`,
/// returning LSP `TextEdit` JSON objects that replace those lines with their
/// formatted equivalents.
///
/// Returns an empty vector when the source is empty, the range is empty, or
/// the selected lines are already correctly formatted.
pub fn range_format(source: &str, start_line: u32, end_line: u32) -> Vec<Value> {
    if source.is_empty() || start_line > end_line {
        return Vec::new();
    }

    let start = start_line as usize;
    let end = end_line as usize;

    // `split('\n')` on "a\nb\n" yields `["a", "b", ""]`; the trailing empty
    // string is not a real content line, so drop it from consideration.
    let all_lines: Vec<&str> = source.split('\n').collect();
    let last_real = all_lines.len().saturating_sub(1);
    // When the source ends with a newline, the final element is an empty
    // string that represents the position *after* the last newline rather
    // than a real line.
    let last_content = if source.ends_with('\n') && all_lines.len() >= 2 {
        last_real.saturating_sub(1)
    } else {
        last_real
    };

    if start > last_content {
        return Vec::new();
    }
    let end = end.min(last_content);
    if start > end {
        return Vec::new();
    }

    // Compute the bracket depth at the start of `start_line` by scanning the
    // document from the very beginning up to (but not including) that line.
    let initial_depth = compute_depth_at_line(source, start);

    // Extract the lines in the selected range.
    let range_lines: Vec<&str> = all_lines[start..=end].to_vec();

    // Run the shared formatting logic on just these lines.
    let formatted = format_lines(&range_lines, initial_depth);

    // If nothing changed, there is nothing to edit.
    let changed = formatted
        .iter()
        .zip(range_lines.iter())
        .any(|(f, o)| f.as_str() != *o);
    if !changed {
        return Vec::new();
    }

    // Build a single `TextEdit` that replaces the whole range. The range end
    // is the start of the line *after* the selection (`end_line + 1`), so the
    // replacement consumes exactly the selected lines and their terminating
    // newline.
    let new_text = format!("{}\n", formatted.join("\n"));

    vec![json!({
        "range": {
            "start": { "line": start_line, "character": 0 },
            "end": { "line": end_line + 1, "character": 0 },
        },
        "newText": new_text,
    })]
}

// ---------------------------------------------------------------------------
// Core formatting logic (mirrors `crate::formatting` but operates on a slice
// of lines with a caller-supplied initial bracket depth).
// ---------------------------------------------------------------------------

/// Compute the net bracket depth at the start of `line_idx` by scanning every
/// line before it. Block-comment state is tracked across lines so that
/// brackets inside block comments are ignored.
fn compute_depth_at_line(source: &str, line_idx: usize) -> i32 {
    let mut depth: i32 = 0;
    let mut in_block = false;
    for (i, line) in source.split('\n').enumerate() {
        if i >= line_idx {
            break;
        }
        let stripped = line.trim_end();
        let (delta, still) = scan_brackets(stripped, in_block);
        in_block = still;
        depth = (depth + delta).max(0);
    }
    depth
}

/// Format a slice of lines, returning the formatted text for each. The
/// `initial_depth` is the bracket depth in effect at the start of the first
/// line, used when the range begins inside a nested block.
fn format_lines(lines: &[&str], initial_depth: i32) -> Vec<String> {
    struct LineInfo {
        indent: usize,
        content: String,
        is_blank: bool,
        is_comment: bool,
    }

    // --- First pass: compute indent depth and trimmed content per line. ---
    let mut depth: i32 = initial_depth;
    let mut in_block_comment = false;
    let mut infos: Vec<LineInfo> = Vec::with_capacity(lines.len());
    for line in lines {
        let stripped = line.trim_end();
        let trimmed = stripped.trim_start();
        let is_blank = trimmed.is_empty();
        let is_comment = in_block_comment || trimmed.starts_with("--");

        let indent = if is_blank {
            0
        } else if starts_with_closing(trimmed) {
            (depth - 1).max(0) as usize
        } else {
            depth as usize
        };

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

    formatted
}

// ---------------------------------------------------------------------------
// Helpers (identical to the private helpers in `crate::formatting`).
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
