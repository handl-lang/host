//! `textDocument/selectionRange` support for the handl LSP server.
//!
//! This module implements smart selection expansion (`Shift+Alt+→` in VSCode)
//! by building, for each cursor position, a linked list of
//! [`SelectionRange`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_selectionRange)
//! objects that grow progressively wider:
//!
//! 1. **Token level** — the lexed token under the cursor.
//! 2. **Bracket pairs** — for every matching `{}`, `()` or `[]` pair that
//!    contains the cursor, two ranges are emitted: the *content* inside the
//!    brackets and the *brackets themselves*. Pairs are sorted smallest-first
//!    so nested brackets expand outward.
//! 3. **Line level** — the entire source line containing the cursor.
//! 4. **Document level** — the whole document.
//!
//! Ranges are emitted in that order and filtered so each parent is strictly
//! wider than its child, yielding a clean monotonically-expanding hierarchy.

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, TokenClasses};

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Compute `textDocument/selectionRange` results.
///
/// For each byte offset in `positions`, return a linked list of LSP
/// `SelectionRange` JSON objects (innermost first) representing progressively
/// wider selections around that cursor. The returned vector has one entry per
/// requested position (in the same order).
pub fn selection_ranges(source: &str, positions: &[usize]) -> Vec<Value> {
    // Empty source → nothing to select.
    if source.is_empty() {
        return positions.iter().map(|_| Value::Null).collect();
    }

    // Lex once and reuse for every position.
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => {
            // If the source cannot be lexed, fall back to line/document-only
            // hierarchies so the feature still degrades gracefully.
            return positions
                .iter()
                .map(|&pos| build_chain(source, &[], pos, &[]))
                .collect();
        }
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => {
            // If the source cannot be lexed, fall back to line/document-only
            // hierarchies so the feature still degrades gracefully.
            return positions
                .iter()
                .map(|&pos| build_chain(source, &[], pos, &[]))
                .collect();
        }
    };

    // Pre-compute every matching bracket pair in the file.
    let classes = TokenClasses::new(kit.language());
    let pairs = bracket_pairs(&classes, &tokens);

    positions
        .iter()
        .map(|&pos| build_chain(source, &tokens, pos, &pairs))
        .collect()
}

// ---------------------------------------------------------------------------
// Bracket pair collection
// ---------------------------------------------------------------------------

/// A matching bracket pair: the byte spans of the opening and closing tokens.
#[derive(Clone, Copy, Debug)]
struct Pair {
    /// Inclusive start offset of the opening bracket.
    open_start: usize,
    /// Exclusive end offset of the opening bracket (just after it).
    open_end: usize,
    /// Inclusive start offset of the closing bracket.
    close_start: usize,
    /// Exclusive end offset of the closing bracket (just after it).
    close_end: usize,
}

impl Pair {
    /// Byte width of the outer range (the brackets themselves).
    fn outer_size(&self) -> usize {
        self.close_end.saturating_sub(self.open_start)
    }

    /// Whether the cursor byte offset lies within this pair's outer range.
    fn contains(&self, pos: usize) -> bool {
        self.open_start <= pos && pos < self.close_end
    }
}

/// Walk the token stream with a stack and collect every matching bracket
/// pair. Bracket kinds are resolved from the language's
/// `open-bracket`/`close-bracket` categories and `pair` links — no hardcoded
/// kind strings.
fn bracket_pairs(classes: &TokenClasses, tokens: &[SpannedToken]) -> Vec<Pair> {
    let mut out = Vec::new();
    // (open kind, open_start, open_end)
    let mut stack: Vec<(String, usize, usize)> = Vec::new();

    for t in tokens {
        let kind = t.kind.as_str();

        if classes.is_open_bracket(kind) {
            stack.push((kind.to_string(), t.span.start, t.span.end));
            continue;
        }

        if classes.is_close_bracket(kind) {
            // Pop until we find the matching opener; discard mismatched ones.
            while let Some((open_kind, open_start, open_end)) = stack.last().cloned() {
                if classes.pair_of(kind) == Some(open_kind.as_str()) {
                    stack.pop();
                    out.push(Pair {
                        open_start,
                        open_end,
                        close_start: t.span.start,
                        close_end: t.span.end,
                    });
                    break;
                } else {
                    stack.pop();
                }
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Per-position hierarchy
// ---------------------------------------------------------------------------

/// Build the linked-list `SelectionRange` JSON for a single cursor position.
fn build_chain(source: &str, tokens: &[SpannedToken], pos: usize, pairs: &[Pair]) -> Value {
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    // a) Token level — the token under the cursor.
    if let Some((start, end)) = token_at(tokens, pos) {
        push_if_wider(&mut ranges, start, end);
    }

    // b) Bracket pairs containing the cursor, smallest first. For each pair
    //    emit the inner (content) range then the outer (brackets) range.
    let mut containing: Vec<Pair> = pairs.iter().copied().filter(|p| p.contains(pos)).collect();
    containing.sort_by_key(|p| p.outer_size());
    for p in containing {
        // Inner: just after the opening bracket to just before the closing one.
        push_if_wider(&mut ranges, p.open_end, p.close_start);
        // Outer: the opening bracket through the closing bracket (inclusive).
        push_if_wider(&mut ranges, p.open_start, p.close_end);
    }

    // c) Line level — the entire line containing the cursor.
    if let Some((line_start, line_end)) = line_span(source, pos) {
        push_if_wider(&mut ranges, line_start, line_end);
    }

    // d) Document level — the whole source.
    push_if_wider(&mut ranges, 0, source.len());

    // Assemble the linked list from outermost (last) to innermost (first).
    let mut parent: Option<Value> = None;
    for &(start, end) in ranges.iter().rev() {
        let mut obj = json!({ "range": span_to_range(source, start, end) });
        if let Some(p) = parent.take() {
            obj["parent"] = p;
        }
        parent = Some(obj);
    }

    parent.unwrap_or_else(|| {
        // No ranges at all (e.g. cursor in empty whitespace) — emit a
        // zero-width range at the cursor position as a fallback.
        json!({ "range": span_to_range(source, pos, pos) })
    })
}

/// Find the token whose span contains `pos` (inclusive start, exclusive end).
/// Falls back to an adjacent token when the cursor sits exactly on a span
/// boundary.
fn token_at(tokens: &[SpannedToken], pos: usize) -> Option<(usize, usize)> {
    // Prefer a token that strictly contains the cursor.
    for t in tokens {
        if t.span.start <= pos && pos < t.span.end {
            return Some((t.span.start, t.span.end));
        }
    }
    // Fall back to a token whose end is exactly the cursor (cursor right after
    // a token), picking the nearest one.
    let mut best: Option<(usize, usize)> = None;
    for t in tokens {
        if t.span.end == pos && t.span.start < t.span.end {
            best = Some((t.span.start, t.span.end));
        }
    }
    best
}

/// Return the `(start, end)` byte offsets of the line containing `pos`.
/// The range covers the line content up to (but excluding) the newline.
fn line_span(source: &str, pos: usize) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let pos = pos.min(source.len());
    // Start: just after the previous newline (or 0).
    let start = bytes[..pos]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    // End: the next newline (exclusive) or end of source.
    let end = bytes[pos..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|i| pos + i)
        .unwrap_or(source.len());
    if end > start {
        Some((start, end))
    } else {
        None
    }
}

/// Append `(start, end)` to `ranges` only if it is strictly wider than the
/// current last range, keeping the hierarchy monotonically expanding.
fn push_if_wider(ranges: &mut Vec<(usize, usize)>, start: usize, end: usize) {
    let size = end.saturating_sub(start);
    if size == 0 {
        return;
    }
    if let Some(&(ls, le)) = ranges.last() {
        if size <= le.saturating_sub(ls) {
            return;
        }
    }
    ranges.push((start, end));
}

// ---------------------------------------------------------------------------
// Offset -> LSP position
// ---------------------------------------------------------------------------

/// Build an LSP `Range` JSON object from byte offsets.
fn span_to_range(source: &str, start: usize, end: usize) -> Value {
    let (start_line, start_char) = offset_to_position(source, start);
    let (end_line, end_char) = offset_to_position(source, end);
    json!({
        "start": { "line": start_line, "character": start_char },
        "end": { "line": end_line, "character": end_char },
    })
}
