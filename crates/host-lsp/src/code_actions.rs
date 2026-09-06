//! `textDocument/codeAction` support for the handl LSP server.
//!
//! Provides quick fixes for common arrow-file issues:
//!
//! * **Wrap in `do { } while` block** — a binding that is not already inside a
//!   `do { … } while` block can be wrapped so that subsequent bindings are
//!   sequenced.
//! * **Add missing `return`** — a match-arm body that is a bare object literal
//!   `{ … }` (without a `return` prefix) gets the prefix inserted.
//! * **Convert `//` to `///` doc comment** — a regular line comment at the top
//!   of the file (before any code) can be promoted to a doc comment.
//! * **Extract to named arrow** — a complex selection containing `>>>`, `|||`,
//!   or a nested `~>` arrow can be extracted into a named arrow binding.

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, is_comment_kind, is_keyword_kind};

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;

/// The URI used for edits in untitled buffers.
const UNTITLED_URI: &str = "file:///untitled.hnl";

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Return LSP CodeAction JSON objects for the given range and diagnostics.
///
/// `range_start`/`range_end` are byte offsets into `source`.  `diagnostics`
/// is the list of published diagnostics (LSP JSON objects) for the document;
/// their messages are inspected to offer diagnostic-driven quick fixes.
///
/// Only actions applicable to the given range/diagnostics are returned.  If
/// none apply, an empty vector is returned.
pub fn code_actions(
    source: &str,
    range_start: usize,
    range_end: usize,
    diagnostics: &[Value],
) -> Vec<Value> {
    let mut actions = Vec::new();

    // The extract action is purely string-based and needs no lexing.
    actions.extend(extract_to_named_arrow_actions(
        source,
        range_start,
        range_end,
    ));

    // Lex the source for token-based actions.
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => return actions,
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return actions,
    };

    // Range-based actions for the user's selection / cursor position.
    actions.extend(wrap_in_do_actions(source, &tokens, range_start, range_end));
    actions.extend(add_return_actions(source, &tokens, range_start, range_end));
    actions.extend(convert_comment_actions(
        source,
        &tokens,
        range_start,
        range_end,
    ));

    // Diagnostic-based actions: if a diagnostic message mentions "return",
    // offer the corresponding fix at the diagnostic's location.
    for diag in diagnostics {
        let msg = diag.get("message").and_then(|m| m.as_str()).unwrap_or("");
        if let Some((ds, de)) = diag_range_to_offsets(source, diag) {
            if msg.contains("return") {
                actions.extend(add_return_actions(source, &tokens, ds, de));
            }
        }
    }

    actions
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Convert a byte offset range into an LSP `Range` JSON object.
fn span_to_range(source: &str, start: usize, end: usize) -> Value {
    let (sl, sc) = offset_to_position(source, start);
    let (el, ec) = offset_to_position(source, end);
    json!({
        "start": { "line": sl, "character": sc },
        "end": { "line": el, "character": ec },
    })
}

/// Build a single CodeAction JSON object with one text edit.
fn make_action(title: &str, source: &str, start: usize, end: usize, new_text: &str) -> Value {
    json!({
        "title": title,
        "kind": "quickfix",
        "edit": {
            "changes": {
                UNTITLED_URI: [{
                    "range": span_to_range(source, start, end),
                    "newText": new_text,
                }]
            }
        },
        "isPreferred": true,
    })
}

/// Return the byte range `[start, end)` of the line containing `offset`,
/// excluding the trailing newline.
fn line_range(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = source[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(source.len());
    (start, end)
}

/// Whether the cursor / selection touches the line containing `token_offset`.
///
/// For a zero-width cursor (`range_start == range_end`) the cursor must lie
/// within the line.  For a non-empty selection the selection must overlap the
/// line.
fn range_touches_line(
    source: &str,
    range_start: usize,
    range_end: usize,
    token_offset: usize,
) -> bool {
    let (line_start, line_end) = line_range(source, token_offset);
    if range_start == range_end {
        range_start >= line_start && range_start <= line_end
    } else {
        range_start < line_end && range_end > line_start
    }
}

/// Convert an LSP diagnostic's `(line, character)` range into byte offsets.
fn diag_range_to_offsets(source: &str, diag: &Value) -> Option<(usize, usize)> {
    let range = diag.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    let sl = start.get("line")?.as_u64()? as u32;
    let sc = start.get("character")?.as_u64()? as u32;
    let el = end.get("line")?.as_u64()? as u32;
    let ec = end.get("character")?.as_u64()? as u32;
    let s = crate::document::position_to_offset(source, sl, sc);
    let e = crate::document::position_to_offset(source, el, ec);
    Some((s, e))
}

// ---------------------------------------------------------------------------
// a. Wrap in `do` block
// ---------------------------------------------------------------------------

/// Whether the byte `offset` is inside a `do { … }` block.
///
/// Walks tokens before `offset`, tracking brace depth.  A `do` keyword
/// followed by `{` pushes the depth onto a stack; the matching `}` pops it.
/// If the stack is non-empty at `offset`, we are inside a do block.
fn is_inside_do_block(tokens: &[SpannedToken], offset: usize) -> bool {
    let mut depth: i32 = 0;
    let mut do_depths: Vec<i32> = Vec::new();
    let mut pending_do = false;
    for t in tokens {
        if t.span.start >= offset {
            break;
        }
        match t.kind.as_str() {
            "token.lbrace" => {
                depth += 1;
                if pending_do {
                    do_depths.push(depth);
                    pending_do = false;
                }
            }
            "token.rbrace" => {
                if do_depths.last() == Some(&depth) {
                    do_depths.pop();
                }
                depth -= 1;
            }
            _ => {
                if is_keyword_kind(t.kind.as_str()) && t.text == "do" {
                    pending_do = true;
                } else {
                    pending_do = false;
                }
            }
        }
    }
    !do_depths.is_empty()
}

/// Offer to wrap a `<-` binding (that is not already inside a `do` block) in
/// `do { … }`.
fn wrap_in_do_actions(
    source: &str,
    tokens: &[SpannedToken],
    range_start: usize,
    range_end: usize,
) -> Vec<Value> {
    let mut actions = Vec::new();
    for i in 0..tokens.len() {
        // `<-` is lexed as two tokens: `token.lt` followed by `token.minus`.
        let t = &tokens[i];
        if !(t.kind == "token.lt" && i + 1 < tokens.len() && tokens[i + 1].kind == "token.minus") {
            continue;
        }
        // The binding must be on the cursor / selection's line.
        if !range_touches_line(source, range_start, range_end, t.span.start) {
            continue;
        }
        // Skip if already inside a do block.
        if is_inside_do_block(tokens, t.span.start) {
            continue;
        }
        // Wrap the entire line in `do { … }`, preserving indentation.
        let (line_start, line_end) = line_range(source, t.span.start);
        let line_text = &source[line_start..line_end];
        let indent_len = line_text.len() - line_text.trim_start().len();
        let indent = &line_text[..indent_len];
        let body = &line_text[indent_len..];
        let new_text = format!("do {{\n{}  {}\n{}}}", indent, body, indent);
        actions.push(make_action(
            "Wrap in `do` block",
            source,
            line_start,
            line_end,
            &new_text,
        ));
        // Only one wrap action per range.
        break;
    }
    actions
}

// ---------------------------------------------------------------------------
// b. Add missing `return`
// ---------------------------------------------------------------------------

/// Offer to insert `return ` before a match-arm object literal `{ … }`
/// that lacks the prefix.
///
/// A `{` is considered a match-arm body when the immediately preceding token
/// is `:` (the arm separator) and the preceding token is not already a
/// `return` keyword.
#[allow(non_snake_case)]
fn add_return_actions(
    source: &str,
    tokens: &[SpannedToken],
    range_start: usize,
    range_end: usize,
) -> Vec<Value> {
    let mut actions = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if t.kind != "token.lbrace" {
            continue;
        }
        // Must be on the cursor / selection's line.
        if !range_touches_line(source, range_start, range_end, t.span.start) {
            continue;
        }
        let prev1 = if i > 0 { tokens.get(i - 1) } else { None };

        // Already has `return` prefix → skip.
        let has_return =
            prev1.map(|p| is_keyword_kind(p.kind.as_str()) && p.text == "return") == Some(true);
        if has_return {
            continue;
        }

        // Must be a match-arm body: preceded by `:`.
        if prev1.map(|p| p.kind.as_str()) != Some("token.colon") {
            continue;
        }

        actions.push(make_action(
            "Add missing return",
            source,
            t.span.start,
            t.span.start,
            "return ",
        ));
        // Only one per line.
        break;
    }
    actions
}

// ---------------------------------------------------------------------------
// c. Convert `//` to `///` doc comment
// ---------------------------------------------------------------------------

/// Whether the comment at `index` is at the top of the file — i.e. every
/// token before it is also a comment (doc or regular).
fn is_top_of_file_comment(tokens: &[SpannedToken], index: usize) -> bool {
    tokens
        .iter()
        .take(index)
        .all(|t| is_comment_kind(t.kind.as_str()))
}

/// Offer to convert a top-of-file `--` comment into a `---` doc comment.
fn convert_comment_actions(
    source: &str,
    tokens: &[SpannedToken],
    range_start: usize,
    range_end: usize,
) -> Vec<Value> {
    let mut actions = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        // Only regular comments (not already doc comments).
        if t.kind != "token.comment" {
            continue;
        }
        // Must be at the top of the file (before any non-comment code).
        if !is_top_of_file_comment(tokens, i) {
            continue;
        }
        // Must be on the cursor / selection's line.
        if !range_touches_line(source, range_start, range_end, t.span.start) {
            continue;
        }
        // Replace the leading `--` with `---`.
        actions.push(make_action(
            "Convert `--` to `---` doc comment",
            source,
            t.span.start,
            t.span.start + 2,
            "---",
        ));
        break;
    }
    actions
}

// ---------------------------------------------------------------------------
// d. Extract to named arrow
// ---------------------------------------------------------------------------

/// Offer to extract a complex selection into a named arrow binding by
/// prepending `name = ` to the selection.
///
/// A selection is "complex" when it contains `>>>`, `|||`, or a nested
/// `~>` expression.  This action requires a non-empty selection.
fn extract_to_named_arrow_actions(
    source: &str,
    range_start: usize,
    range_end: usize,
) -> Vec<Value> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut start = range_start.min(source.len());
    let mut end = range_end.min(source.len());
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    if start == end {
        return Vec::new();
    }
    // Ensure char boundaries.
    while start < source.len() && !source.is_char_boundary(start) {
        start += 1;
    }
    while end < source.len() && !source.is_char_boundary(end) {
        end += 1;
    }
    if start >= end {
        return Vec::new();
    }
    let selection = &source[start..end];
    if selection.contains(">>>") || selection.contains("|||") || selection.contains("~>") {
        vec![make_action(
            "Extract to named arrow",
            source,
            start,
            start,
            "name = ",
        )]
    } else {
        Vec::new()
    }
}
