//! `textDocument/documentSymbol` support for the handl LSP server.
//!
//! This module lexes a `.hnl` source file and produces a flat list of
//! [`DocumentSymbol`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_documentSymbol)
//! JSON objects describing the structure of the file, suitable for display in
//! VSCode's outline panel.
//!
//! Three flavours of `.hnl` file are recognised:
//!
//! * **Arrow files** — the first significant token is the `rec` keyword.
//!   Symbols are produced for the rec/while/for blocks, match-arm patterns,
//!   object keys and bindings.
//! * **Contract files** — the source contains a `law` keyword. Each
//!   `law name : signature` becomes an interface symbol.
//! * **General files** — everything else. Top-level bindings, `match` and `if`
//!   expressions become symbols.

use serde_json::{Value, json};

use handl_kit_canonical::{
    SpannedToken, is_comment_kind, is_identifier_kind, is_keyword_kind,
};

use crate::kit_resolver::kit_for_file;
use crate::line_map::LineMap;

// ---------------------------------------------------------------------------
// LSP SymbolKind constants
// ---------------------------------------------------------------------------

/// SymbolKind.Field
const KIND_FIELD: u32 = 8;
/// SymbolKind.Interface
const KIND_INTERFACE: u32 = 11;
/// SymbolKind.Variable
const KIND_VARIABLE: u32 = 13;
/// SymbolKind.Function
const KIND_FUNCTION: u32 = 23;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Lex `source` and return a vector of DocumentSymbol JSON objects.
///
/// Returns an empty vector if the source cannot be lexed.
pub fn document_symbols(source: &str) -> Vec<Value> {
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    // Pre-compute line starts once so every offset→position query is O(log n)
    // instead of an O(n) scan from the start of the source.
    let lines = LineMap::new(source);

    // The first significant (non-comment) token decides whether this is an
    // arrow file (it starts with the `rec` keyword).
    let is_arrow = tokens
        .iter()
        .find(|t| !is_comment_kind(&t.kind))
        .map(|t| is_keyword_kind(&t.kind) && t.text == "rec")
        .unwrap_or(false);

    // Contract files are identified by the presence of a `law` keyword.
    let is_contract = tokens
        .iter()
        .any(|t| is_keyword_kind(&t.kind) && t.text == "law");

    if is_arrow {
        arrow_symbols(source, &lines, &tokens)
    } else if is_contract {
        contract_symbols(source, &lines, &tokens)
    } else {
        general_symbols(source, &lines, &tokens)
    }
}

// ---------------------------------------------------------------------------
// Arrow files
// ---------------------------------------------------------------------------

/// The kind of bracket currently open on the stack. Only braces carry
/// semantic distinction (match body vs. record literal); parens and brackets
/// are tracked purely so that `,` and `:` inside them are not mistaken for
/// match-arm separators.
#[derive(Clone, Copy, PartialEq)]
enum Frame {
    Paren,
    Bracket,
    BraceMatch,
    BraceObject,
}

fn arrow_symbols(source: &str, lines: &LineMap, tokens: &[SpannedToken]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    // Set by the `match` keyword, consumed by the next `{` (which opens a
    // match body rather than a record literal).
    let mut expect_match_body = false;
    // Token index of the first token of the current match-arm pattern, when
    // we are directly inside a match body and in pattern position.
    let mut pattern_start: Option<usize> = None;

    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        let kind = t.kind.as_str();

        if is_comment_kind(kind) {
            i += 1;
            continue;
        }

        // Keywords -----------------------------------------------------------
        if is_keyword_kind(kind) {
            match t.text.as_str() {
                "rec" => out.push(sym(
                    "rec",
                    KIND_FUNCTION,
                    "feedback loop",
                    lines,
                    t.span.start,
                    t.span.end,
                )),
                "while" => out.push(sym(
                    "while",
                    KIND_FUNCTION,
                    "while loop",
                    lines,
                    t.span.start,
                    t.span.end,
                )),
                "for" => out.push(sym(
                    "for",
                    KIND_FUNCTION,
                    "for loop",
                    lines,
                    t.span.start,
                    t.span.end,
                )),
                "match" => {
                    // The `match` keyword itself is not a symbol in arrow
                    // files, but its body opens a match frame so that arm
                    // patterns can be collected.
                    expect_match_body = true;
                }
                _ => {}
            }
            i += 1;
            continue;
        }

        // Brackets / braces / parens ----------------------------------------
        match kind {
            "token.lbrace" => {
                let frame = if expect_match_body {
                    expect_match_body = false;
                    pattern_start = next_sig_idx(tokens, i + 1);
                    Frame::BraceMatch
                } else {
                    Frame::BraceObject
                };
                stack.push(frame);
            }
            "token.rbrace" => {
                stack.pop();
                pattern_start = None;
            }
            "token.lparen" => {
                stack.push(Frame::Paren);
            }
            "token.rparen" => {
                stack.pop();
            }
            "token.lbracket" => {
                stack.push(Frame::Bracket);
            }
            "token.rbracket" => {
                stack.pop();
            }
            "token.colon" => {
                // A `:` directly inside a match body separates the arm
                // pattern from the arm body.
                if matches!(stack.last(), Some(Frame::BraceMatch)) {
                    if let Some(start_idx) = pattern_start {
                        if start_idx < i {
                            let pstart = tokens[start_idx].span.start;
                            let mut pend = pstart;
                            for j in start_idx..i {
                                if !is_comment_kind(&tokens[j].kind) {
                                    pend = tokens[j].span.end;
                                }
                            }
                            // Use the original source text so the pattern
                            // keeps its formatting (e.g. "[h, ...t]").
                            let name = &source[pstart..pend];
                            out.push(sym(
                                name,
                                KIND_VARIABLE,
                                "match arm",
                                lines,
                                pstart,
                                pend,
                            ));
                        }
                        pattern_start = None;
                    }
                }
            }
            "token.comma" => {
                // A `,` directly inside a match body ends an arm body and
                // returns to pattern position for the next arm.
                if matches!(stack.last(), Some(Frame::BraceMatch)) {
                    pattern_start = next_sig_idx(tokens, i + 1);
                }
            }
            _ => {
                // Object key: an identifier directly inside a record literal
                // followed by `:` or `=` is a field name. v0.1 syntax uses
                // `=` for field assignment (e.g. `elem = input.0`), while
                // older syntax uses `:` (e.g. `elem: input.0`).
                if is_identifier_kind(kind) {
                    if matches!(stack.last(), Some(Frame::BraceObject)) {
                        if let Some(ni) = next_sig_idx(tokens, i + 1) {
                            if tokens[ni].kind == "token.colon"
                                || tokens[ni].kind == "token.equal"
                            {
                                out.push(sym(
                                    &t.text,
                                    KIND_FIELD,
                                    "field",
                                    lines,
                                    t.span.start,
                                    t.span.end,
                                ));
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    out
}

// ---------------------------------------------------------------------------
// Contract files
// ---------------------------------------------------------------------------

fn contract_symbols(source: &str, lines: &LineMap, tokens: &[SpannedToken]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        if is_keyword_kind(&t.kind) && t.text == "law" {
            // Collect the law name (possibly dotted, e.g. `list.head`) up
            // to the `:` that introduces the signature.
            let mut j = i + 1;
            let mut name_start: Option<usize> = None;
            let mut name_end = t.span.end;
            let mut colon_end: Option<usize> = None;
            while j < tokens.len() {
                let tj = &tokens[j];
                if is_comment_kind(&tj.kind) {
                    j += 1;
                    continue;
                }
                if tj.kind == "token.colon" {
                    colon_end = Some(tj.span.end);
                    break;
                }
                // Another `law` without a colon — bail out.
                if is_keyword_kind(&tj.kind) && tj.text == "law" {
                    break;
                }
                if is_identifier_kind(&tj.kind) || tj.kind == "token.dot" {
                    if name_start.is_none() {
                        name_start = Some(tj.span.start);
                    }
                    name_end = tj.span.end;
                    j += 1;
                } else {
                    break;
                }
            }

            if let (Some(ns), Some(ce)) = (name_start, colon_end) {
                let name = &source[ns..name_end];
                let sig = lines.rest_of_line(source, ce);
                let line_end = lines.line_end_offset(ce);
                out.push(symbol_full(
                    name,
                    KIND_INTERFACE,
                    sig,
                    lines,
                    t.span.start,
                    line_end,
                    ns,
                    name_end,
                ));
            }
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// General .hnl files
// ---------------------------------------------------------------------------

fn general_symbols(source: &str, lines: &LineMap, tokens: &[SpannedToken]) -> Vec<Value> {
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        let kind = t.kind.as_str();
        if is_comment_kind(kind) {
            continue;
        }
        if is_keyword_kind(kind) {
            match t.text.as_str() {
                "match" => out.push(sym(
                    "match",
                    KIND_FUNCTION,
                    "pattern match",
                    lines,
                    t.span.start,
                    t.span.end,
                )),
                "if" => out.push(sym(
                    "if",
                    KIND_FUNCTION,
                    "conditional",
                    lines,
                    t.span.start,
                    t.span.end,
                )),
                "while" => out.push(sym(
                    "while",
                    KIND_FUNCTION,
                    "while loop",
                    lines,
                    t.span.start,
                    t.span.end,
                )),
                "for" => out.push(sym(
                    "for",
                    KIND_FUNCTION,
                    "for loop",
                    lines,
                    t.span.start,
                    t.span.end,
                )),
                "import" => {
                    // `import name from module` — the next significant token
                    // is the imported name.
                    if let Some(ni) = next_sig_idx(tokens, i + 1) {
                        let nt = &tokens[ni];
                        if is_identifier_kind(&nt.kind) {
                            out.push(sym(
                                &nt.text,
                                KIND_VARIABLE,
                                "import",
                                lines,
                                nt.span.start,
                                nt.span.end,
                            ));
                        }
                    }
                }
                "trait" => {
                    // `trait Name { … }` — the next significant token is the
                    // trait name.
                    if let Some(ni) = next_sig_idx(tokens, i + 1) {
                        let nt = &tokens[ni];
                        if is_identifier_kind(&nt.kind) {
                            out.push(sym(
                                &nt.text,
                                KIND_INTERFACE,
                                "trait",
                                lines,
                                nt.span.start,
                                nt.span.end,
                            ));
                        }
                    }
                }
                "impl" => {
                    // `impl Trait for Type { … }` — the next significant token
                    // is the trait being implemented.
                    if let Some(ni) = next_sig_idx(tokens, i + 1) {
                        let nt = &tokens[ni];
                        if is_identifier_kind(&nt.kind) {
                            out.push(sym(
                                &nt.text,
                                KIND_INTERFACE,
                                "impl",
                                lines,
                                nt.span.start,
                                nt.span.end,
                            ));
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        if is_identifier_kind(kind) {
            // A binding declaration at the start of a line: `name = value` or
            // `name: type`.
            if lines.is_at_line_start(t.span.start) {
                if let Some(ni) = next_sig_idx(tokens, i + 1) {
                    let nk = tokens[ni].kind.as_str();
                    if nk == "token.equal" || nk == "token.colon" {
                        out.push(sym(
                            &t.text,
                            KIND_VARIABLE,
                            "binding",
                            lines,
                            t.span.start,
                            t.span.end,
                        ));
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Index of the next non-comment token at or after `from`, or `None`.
fn next_sig_idx(tokens: &[SpannedToken], from: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, t)| !is_comment_kind(&t.kind))
        .map(|(idx, _)| idx)
}

/// Build a DocumentSymbol JSON object where the range and selectionRange are
/// identical (covering `[start, end)`).
fn sym(name: &str, kind: u32, detail: &str, lines: &LineMap, start: usize, end: usize) -> Value {
    symbol_full(name, kind, detail, lines, start, end, start, end)
}

/// Build a DocumentSymbol JSON object with separate range and selectionRange.
fn symbol_full(
    name: &str,
    kind: u32,
    detail: &str,
    lines: &LineMap,
    range_start: usize,
    range_end: usize,
    sel_start: usize,
    sel_end: usize,
) -> Value {
    json!({
        "name": name,
        "kind": kind,
        "detail": detail,
        "range": range_from_span(lines, range_start, range_end),
        "selectionRange": range_from_span(lines, sel_start, sel_end),
    })
}

/// Convert a byte span to an LSP range JSON object.
fn range_from_span(lines: &LineMap, start: usize, end: usize) -> Value {
    let (sl, sc) = lines.position(start);
    let (el, ec) = lines.position(end);
    json!({
        "start": { "line": sl, "character": sc },
        "end": { "line": el, "character": ec },
    })
}
