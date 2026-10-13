//! Range formatting for the `textDocument/rangeFormatting` LSP request.
//!
//! The LSP owns no formatter (ADR-0126, maintainer ruling): formatting is the
//! generic engine's job. Just as `textDocument/formatting` delegates to
//! `plugin-ext-fmt`, range formatting is a **projection** of the same canonical
//! result — format the whole document, then emit an edit covering only the
//! selected lines that changed.
//!
//! This replaced a hand-rolled line-based indenter that duplicated the
//! full-document formatter *and disagreed with it*: it indented input that
//! `hnl fmt` leaves untouched, so "format selection" and "format document"
//! produced different files. See `project/parked.yaml` and ADR-0126.

use handl_plugin_ext_fmt::format_source;
use serde_json::{Value, json};

/// Format the lines in `[start_line, end_line]` (inclusive) of `source`,
/// returning LSP `TextEdit` objects that replace those lines with their
/// canonical equivalents.
///
/// Returns an empty vector when:
/// - the source is empty or the range is inverted or past the end;
/// - the canonical formatter rejects the source (the LSP reports no edit rather
///   than guessing, matching `textDocument/formatting`);
/// - the selected lines are already canonical; or
/// - formatting would change the *number* of lines, which a line-range edit
///   cannot express (the client should format the whole document instead).
pub fn range_format(source: &str, start_line: u32, end_line: u32) -> Vec<Value> {
    if source.is_empty() || start_line > end_line {
        return Vec::new();
    }
    let Ok(formatted) = format_source(source) else {
        return Vec::new();
    };
    if formatted == source {
        return Vec::new();
    }

    let before: Vec<&str> = source.split('\n').collect();
    let last = before.len().saturating_sub(1);

    // Whole document (or a range extending past it): emit one edit covering
    // everything, shaped exactly like `textDocument/formatting`
    // (`format_document_json`), so the two requests cannot disagree.
    if start_line == 0 && (end_line as usize) >= last {
        let end_char = before.last().map(|s| s.chars().count()).unwrap_or(0);
        return vec![json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": last, "character": end_char },
            },
            "newText": formatted,
        })];
    }

    // Partial range: a line-range edit can only express a same-shape change.
    let after: Vec<&str> = formatted.split('\n').collect();
    if before.len() != after.len() {
        return Vec::new();
    }
    let start = start_line as usize;
    let end = (end_line as usize).min(last);
    if start > end || start >= before.len() {
        return Vec::new();
    }

    // Nothing in the selection actually changed.
    if (start..=end).all(|i| before[i] == after[i]) {
        return Vec::new();
    }

    // The replaced span is `[start_of_line(start), start_of_line(end + 1))`, so
    // the replacement carries the terminating newline unless it runs to EOF.
    let joined = after[start..=end].join("\n");
    let new_text = if end + 1 < after.len() {
        format!("{joined}\n")
    } else {
        joined
    };
    vec![json!({
        "range": {
            "start": { "line": start_line, "character": 0 },
            "end": { "line": end_line + 1, "character": 0 },
        },
        "newText": new_text,
    })]
}
