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

use handl_kit_canonical::{SpannedToken, is_doc_comment_kind};
use handl_language::{Language, TokenCategory as TC};

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
    let lang = kit.lang();
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut ranges = Vec::new();

    // Pre-compute line starts once so every offset→position query is O(log n)
    // instead of an O(n) scan from the start of the source.
    let lines = LineMap::new(source);

    // Bracket-pair folding ranges.
    ranges.extend(bracket_ranges(lang, &lines, &tokens));

    // Comment-block folding ranges.
    ranges.extend(comment_ranges(lang, source, &lines, &tokens));

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
/// Bracket identity comes from `lang.tokens` categories (ADR-0081).
fn bracket_ranges(lang: &Language, lines: &LineMap, tokens: &[SpannedToken]) -> Vec<Value> {
    let mut out = Vec::new();
    // Stack of (open bracket category, byte offset of the opening token).
    let mut stack: Vec<(TC, usize)> = Vec::new();

    for t in tokens {
        let Some(cat) = lang.token_category(&t.kind) else {
            continue;
        };

        if cat.is_open_bracket() {
            stack.push((cat, t.span.start));
            continue;
        }

        // On a closing bracket, pop the nearest matching opener. If the top
        // of the stack does not match we simply drop it (the source is
        // malformed); well-formed input is the common case.
        if cat.is_close_bracket() {
            while let Some(&(open_cat, open_offset)) = stack.last() {
                if open_cat.matching_close() == Some(cat) {
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
    lang: &Language,
    source: &str,
    lines: &LineMap,
    tokens: &[SpannedToken],
) -> Vec<Value> {
    let mut out = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        let cat = lang.token_category(&tokens[i].kind);
        if !cat.map(|c| c.is_comment_like()).unwrap_or(false) {
            i += 1;
            continue;
        }

        let is_doc = is_doc_comment_kind(lang, &tokens[i].kind);
        // Collect the contiguous run of comment tokens with the same flavour.
        let run_start = tokens[i].span.start;
        let mut run_end = tokens[i].span.end;
        let mut j = i;
        while j < tokens.len() {
            let kj = &tokens[j].kind;
            if !is_comment_like(lang, kj) || is_doc_comment_kind(lang, kj) != is_doc {
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

/// Whether a token kind is classified as comment trivia (doc or regular)
/// in `lang` (ADR-0081).
fn is_comment_like(lang: &Language, kind: &str) -> bool {
    lang.token_category(kind)
        .map(|c| c.is_comment_like())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Offset -> line/character
// ---------------------------------------------------------------------------
//
// Offset→position conversion is now provided by `crate::line_map::LineMap`,
// which pre-computes line starts once and answers queries in O(log n).
