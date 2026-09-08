//! `textDocument/foldingRange` support for the handl LSP server.
//!
//! This module lexes a `.hnl` source file and produces a flat list of
//! [`FoldingRange`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_foldingRange)
//! JSON objects so that VSCode can collapse `loop`, `match` and
//! object-literal blocks.
//!
//! Two sources of folding ranges are emitted:
//!
//! * **Bracket pairs** — matching `{ }`, `( )` and `[ ]` pairs are tracked
//!   with a stack. Every pair that spans more than one line becomes a
//!   folding range of kind `"region"`.
//! * **Comment blocks** — runs of consecutive doc-comment (`///`) tokens or
//!   regular-comment (`//`) tokens become folding ranges of kind
//!   `"comment"` (again only when they span more than one line).

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, TokenClasses};

use crate::kit_resolver::kit_for_file;
use crate::line_map::LineMap;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Lex `source` and return a vector of LSP FoldingRange JSON objects.
///
/// Returns an empty vector if the source cannot be lexed.
pub fn folding_ranges(source: &str) -> Vec<Value> {
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut ranges = Vec::new();

    // Pre-compute line starts once so every offset→position query is O(log n)
    // instead of an O(n) scan from the start of the source.
    let lines = LineMap::new(source);

    let classes = TokenClasses::new(kit.language());

    // Bracket-pair folding ranges.
    ranges.extend(bracket_ranges(&classes, &lines, &tokens));

    // Comment-block folding ranges.
    ranges.extend(comment_ranges(&classes, source, &lines, &tokens));

    // Sort by start line (then start character for stability).
    ranges.sort_by(|a, b| {
        let al = a["startLine"].as_u64().unwrap_or(0);
        let bl = b["startLine"].as_u64().unwrap_or(0);
        let ac = a["startCharacter"].as_u64().unwrap_or(0);
        let bc = b["startCharacter"].as_u64().unwrap_or(0);
        (al, ac).cmp(&(bl, bc))
    });

    ranges
}

// ---------------------------------------------------------------------------
// Bracket pairs
// ---------------------------------------------------------------------------

/// Walk the token stream tracking matching bracket pairs with a stack and
/// return a folding range for every pair that spans more than one line.
/// Bracket kinds come from the language's `open-bracket`/`close-bracket`
/// categories and `pair` links.
fn bracket_ranges(classes: &TokenClasses, lines: &LineMap, tokens: &[SpannedToken]) -> Vec<Value> {
    let mut out = Vec::new();
    // Stack of (open kind, byte offset of the opening token).
    let mut stack: Vec<(String, usize)> = Vec::new();

    for t in tokens {
        let kind = t.kind.as_str();

        if classes.is_open_bracket(kind) {
            stack.push((kind.to_string(), t.span.start));
            continue;
        }

        // On a closing bracket, pop the nearest matching opener. If the top
        // of the stack does not match we simply drop it (the source is
        // malformed); well-formed input is the common case.
        if classes.is_close_bracket(kind) {
            while let Some((open_kind, open_offset)) = stack.last().cloned() {
                if classes.pair_of(kind) == Some(open_kind.as_str()) {
                    stack.pop();
                    let (start_line, start_char) = lines.position(open_offset);
                    let (end_line, end_char) = lines.position(t.span.end);
                    if end_line > start_line {
                        out.push(json!({
                            "startLine": start_line,
                            "endLine": end_line,
                            "startCharacter": start_char,
                            "endCharacter": end_char,
                            "kind": "region",
                        }));
                    }
                    break;
                } else {
                    // Mismatched opener — discard and keep looking.
                    stack.pop();
                }
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Comment blocks
// ---------------------------------------------------------------------------

/// Group consecutive comment tokens of the same flavour (doc-comment vs
/// regular comment) and emit a `"comment"` folding range for each run that
/// spans more than one line.
fn comment_ranges(
    classes: &TokenClasses,
    source: &str,
    lines: &LineMap,
    tokens: &[SpannedToken],
) -> Vec<Value> {
    let mut out = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        let kind = tokens[i].kind.as_str();
        if !classes.is_comment(kind) {
            i += 1;
            continue;
        }

        let is_doc = classes.is_doc_comment(kind);
        // Collect the contiguous run of comment tokens with the same flavour.
        let run_start = tokens[i].span.start;
        let mut run_end = tokens[i].span.end;
        let mut j = i;
        while j < tokens.len() {
            let kj = tokens[j].kind.as_str();
            if !classes.is_comment(kj) || classes.is_doc_comment(kj) != is_doc {
                break;
            }
            run_end = tokens[j].span.end;
            j += 1;
        }

        // Comment tokens include their trailing newline, which would push the
        // end position onto the following line. Trim trailing line breaks so
        // `endLine` lands on the last comment line (mirroring how a bracket
        // pair's `endLine` lands on the line of the closing bracket).
        let bytes = source.as_bytes();
        while run_end > run_start
            && bytes
                .get(run_end - 1)
                .map_or(false, |&b| b == b'\n' || b == b'\r')
        {
            run_end -= 1;
        }

        let (start_line, start_char) = lines.position(run_start);
        let (end_line, end_char) = lines.position(run_end);
        if end_line > start_line {
            out.push(json!({
                "startLine": start_line,
                "endLine": end_line,
                "startCharacter": start_char,
                "endCharacter": end_char,
                "kind": "comment",
            }));
        }

        i = j;
    }

    out
}

// ---------------------------------------------------------------------------
// Offset -> line/character
// ---------------------------------------------------------------------------
//
// Offset→position conversion is now provided by `crate::line_map::LineMap`,
// which pre-computes line starts once and answers queries in O(log n).
