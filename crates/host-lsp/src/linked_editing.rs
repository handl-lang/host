//! `textDocument/linkedEditingRange` support for the handl LSP server.
//!
//! When the cursor rests on a bracket (`{`, `}`, `(`, `)`, `[`, `]`) the
//! matching bracket's range is returned alongside the cursor's own bracket so
//! that VSCode keeps both ends of the pair in sync while the user types.
//!
//! When the cursor rests on an identifier (e.g. an arrow name), every
//! occurrence of that identifier in the file is returned so that renaming
//! via linked editing stays consistent within the document.
//!
//! The LSP response shape is:
//!
//! ```json
//! { "ranges": [ { "start": {...}, "end": {...} }, ... ] }
//! ```

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, TokenClasses};

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;
use crate::symbols::{find_references, identifier_at_offset};

/// Compute `textDocument/linkedEditingRange` results for the byte `offset`
/// in `source`.
///
/// Returns `None` when the cursor is not on a bracket or identifier. Otherwise
/// returns a JSON object with a `ranges` array of LSP `Range` objects covering
/// every linked location (the cursor's own token plus its matches).
pub fn linked_editing_ranges(source: &str, offset: usize) -> Option<Value> {
    if source.is_empty() {
        return None;
    }

    let kit = kit_for_file(None).ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());

    // 1. Bracket case: cursor on a bracket token.
    if let Some(bracket_token) = bracket_at(&classes, &tokens, offset) {
        let pairs = bracket_pairs(&classes, &tokens);
        // Find the pair that involves this bracket token.
        for p in &pairs {
            let (other_start, other_end) = if bracket_token.span.start == p.open_start {
                (p.close_start, p.close_end)
            } else if bracket_token.span.start == p.close_start {
                (p.open_start, p.open_end)
            } else {
                continue;
            };
            let ranges = vec![
                span_to_range(source, bracket_token.span.start, bracket_token.span.end),
                span_to_range(source, other_start, other_end),
            ];
            return Some(json!({ "ranges": ranges }));
        }
        // An unmatched bracket: link only to itself.
        let range = span_to_range(source, bracket_token.span.start, bracket_token.span.end);
        return Some(json!({ "ranges": [range] }));
    }

    // 2. Identifier case: cursor on an identifier (e.g. an arrow name).
    if let Some((name, _)) = identifier_at_offset(source, offset) {
        let references = find_references(source, &name);
        if references.is_empty() {
            return None;
        }
        let ranges: Vec<Value> = references
            .iter()
            .map(|span| span_to_range(source, span.start, span.end))
            .collect();
        return Some(json!({ "ranges": ranges }));
    }

    None
}

// ---------------------------------------------------------------------------
// Bracket helpers
// ---------------------------------------------------------------------------

/// A matching bracket pair: byte spans of the opening and closing tokens.
struct Pair {
    open_start: usize,
    open_end: usize,
    close_start: usize,
    close_end: usize,
}

/// Return the bracket token whose span contains `offset`, if any.
/// Bracket kinds come from the language's `open-bracket`/`close-bracket`
/// categories — no hardcoded kind strings.
fn bracket_at<'a>(
    classes: &TokenClasses,
    tokens: &'a [SpannedToken],
    offset: usize,
) -> Option<&'a SpannedToken> {
    for t in tokens {
        if classes.is_bracket(&t.kind) && t.span.start <= offset && offset < t.span.end {
            return Some(t);
        }
    }
    // Fall back to a bracket whose end is exactly the cursor.
    for t in tokens {
        if classes.is_bracket(&t.kind) && t.span.end == offset && t.span.start < t.span.end {
            return Some(t);
        }
    }
    None
}

/// Walk the token stream with a stack and collect every matching bracket
/// pair, resolved via `open-bracket`/`close-bracket` categories and `pair`
/// links.
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
