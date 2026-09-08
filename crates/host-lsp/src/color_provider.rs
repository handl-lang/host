//! `textDocument/documentColor` and `textDocument/colorPresentation`
//! support for the handl LSP server.
//!
//! Handl source files (`.hnl`) may contain color literals in two forms:
//!
//! 1. **Hex color literals** — `#rrggbb` or `#rgb` (e.g. `#ff0000`, `#f00`).
//!    These are scanned directly in the raw source because the kit lexer may
//!    not tokenize a `#`-prefixed hex run as a single token.
//! 2. **Named color tokens** — `color("red")`, `color("blue")`, etc. These
//!    are string literals inside `color(...)` calls, detected via the kit
//!    lexer so that comments and nested strings are skipped correctly.
//!
//! The LSP color protocol represents colors as floats in the range 0.0–1.0
//! for the red, green, blue, and alpha channels.

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, is_comment_kind, is_identifier_kind, is_string_kind};
use handl_language::Language;

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;

/// Named colors recognized inside `color("name")` calls.
///
/// Maps a lowercase color name to its 24-bit RGB hex value.
fn named_color(name: &str) -> Option<(u8, u8, u8)> {
    let hex = match name {
        "red" => 0xff0000,
        "green" => 0x00ff00,
        "blue" => 0x0000ff,
        "white" => 0xffffff,
        "black" => 0x000000,
        "yellow" => 0xffff00,
        "cyan" => 0x00ffff,
        "magenta" => 0xff00ff,
        "orange" => 0xffa500,
        "purple" => 0x800080,
        _ => return None,
    };
    Some((
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    ))
}

/// Compute `textDocument/documentColor` results for `source`.
///
/// Scans the source for hex color literals (`#rrggbb` / `#rgb`) and named
/// color calls (`color("red")`), returning one LSP `ColorInformation` JSON
/// object per match:
///
/// ```json
/// {
///   "range": { "start": {"line": L, "character": C1},
///              "end":   {"line": L, "character": C2} },
///   "color": { "red": 0.0, "green": 0.0, "blue": 0.0, "alpha": 1.0 }
/// }
/// ```
///
/// `file_path` is accepted for API symmetry with other document-feature
/// modules but does not affect color extraction — colors are purely lexical.
pub fn document_colors(source: &str, file_path: Option<&str>) -> Vec<Value> {
    let _ = file_path;
    let mut out = Vec::new();

    // Hex literals are scanned directly in the raw source because the kit
    // lexer may not produce a single token for a `#`-prefixed hex run.
    out.extend(hex_color_infos(source));

    // Named colors (`color("name")`) are detected via the kit lexer so that
    // comments and nested strings are skipped correctly.
    out.extend(named_color_infos(source));

    out
}

/// Find `#rrggbb` and `#rgb` hex color literals in `source`.
fn hex_color_infos(source: &str) -> Vec<Value> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'#' {
            i += 1;
            continue;
        }
        // Read the run of hex digits following `#`.
        let start = i;
        let mut j = i + 1;
        while j < bytes.len() && is_hex_digit(bytes[j]) {
            j += 1;
        }
        let count = j - (i + 1);
        // Accept only exactly 3 or 6 hex digits, and require that the
        // character before `#` is not an identifier/hex character so that
        // we don't match the middle of a longer token.
        if (count == 3 || count == 6) && hash_is_standalone(bytes, start) {
            let hex_str = &source[i + 1..j];
            let (r, g, b) = parse_hex(hex_str);
            out.push(color_info(source, start, j, r, g, b, 1.0));
        }
        // Advance past the `#` and any hex digits we consumed.
        i = j.max(i + 1);
    }
    out
}

/// Whether the `#` at `pos` is not preceded by an alphanumeric/underscore
/// character (so it is not part of a longer identifier-like token).
fn hash_is_standalone(bytes: &[u8], pos: usize) -> bool {
    if pos == 0 {
        return true;
    }
    let prev = bytes[pos - 1];
    !(prev.is_ascii_alphanumeric() || prev == b'_')
}

/// Whether `b` is a hexadecimal digit (ASCII).
fn is_hex_digit(b: u8) -> bool {
    b.is_ascii_digit() || (b >= b'a' && b <= b'f') || (b >= b'A' && b <= b'F')
}

/// Parse a 3- or 6-digit hex string into an `(r, g, b)` tuple.
///
/// A 3-digit hex string is expanded by doubling each digit (`#f00` →
/// `#ff0000`).
fn parse_hex(hex: &str) -> (u8, u8, u8) {
    if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0);
        (r, g, b)
    } else if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r, g, b)
    } else {
        (0, 0, 0)
    }
}

/// Find `color("name")` calls in `source` via the kit lexer and emit
/// `ColorInformation` objects for recognized named colors.
fn named_color_infos(source: &str) -> Vec<Value> {
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    // The kit lexer has no rule for `#`, so any hex color literal (`#ff0000`)
    // would make `lex_with_spans` fail and hide the named-color calls. Replace
    // every `#` with a space (preserving byte offsets) so the lexer succeeds;
    // spans still map back to the original `source` because positions are
    // unchanged.
    let stripped: String = source
        .chars()
        .map(|c| if c == '#' { ' ' } else { c })
        .collect();
    let lang = kit.lang();
    let tokens = match kit.lex_with_spans(&stripped) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        // Look for an identifier `color` ...
        if is_identifier_kind(lang, &tokens[i].kind) && tokens[i].text == "color" {
            // ... immediately followed by `(` (skipping comments) ...
            if let Some(open_paren) = next_significant(lang, &tokens, i + 1) {
                if tokens[open_paren].text == "(" {
                    // ... then a string literal ...
                    if let Some(str_idx) = next_significant(lang, &tokens, open_paren + 1) {
                        if is_string_kind(lang, &tokens[str_idx].kind) {
                            // ... then `)`.
                            if let Some(close_idx) = next_significant(lang, &tokens, str_idx + 1) {
                                if tokens[close_idx].text == ")" {
                                    let inner = string_inner(&tokens[str_idx]);
                                    if let Some((r, g, b)) =
                                        named_color(&inner.to_ascii_lowercase())
                                    {
                                        out.push(color_info(
                                            source,
                                            tokens[str_idx].span.start,
                                            tokens[str_idx].span.end,
                                            r,
                                            g,
                                            b,
                                            1.0,
                                        ));
                                    }
                                }
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

/// Return the index of the next non-comment token after `start`, or `None`.
fn next_significant(lang: &Language, tokens: &[SpannedToken], start: usize) -> Option<usize> {
    let mut j = start;
    while j < tokens.len() {
        if !is_comment_kind(lang, &tokens[j].kind) {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Extract the inner content of a string-literal token (without quotes).
fn string_inner(token: &SpannedToken) -> String {
    let s = &token.text;
    let len = s.len();
    if len >= 2 {
        s[1..len - 1].to_string()
    } else {
        String::new()
    }
}

/// Build a single `ColorInformation` JSON object.
fn color_info(source: &str, start: usize, end: usize, r: u8, g: u8, b: u8, alpha: f64) -> Value {
    let (start_line, start_char) = offset_to_position(source, start);
    let (end_line, end_char) = offset_to_position(source, end);
    json!({
        "range": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": end_line, "character": end_char },
        },
        "color": {
            "red": r as f64 / 255.0,
            "green": g as f64 / 255.0,
            "blue": b as f64 / 255.0,
            "alpha": alpha,
        },
    })
}

/// Compute `textDocument/colorPresentation` results for a given color.
///
/// Returns a list of `ColorPresentation` JSON objects, each with a `label`:
///
/// - Hex form: `#rrggbb` (e.g. `#ff0000`)
/// - RGB form: `rgb(r, g, b)` (e.g. `rgb(255, 0, 0)`)
///
/// `range` is accepted per the LSP protocol (it may be used to trim the
/// presentation text) but is not currently used to alter the labels.
pub fn color_presentations(color: &Value, range: &Value) -> Vec<Value> {
    let _ = range;
    let r = color.get("red").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let g = color.get("green").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let b = color.get("blue").and_then(|v| v.as_f64()).unwrap_or(0.0);

    let ri = (r * 255.0).round().clamp(0.0, 255.0) as u8;
    let gi = (g * 255.0).round().clamp(0.0, 255.0) as u8;
    let bi = (b * 255.0).round().clamp(0.0, 255.0) as u8;

    let hex_label = format!("#{:02x}{:02x}{:02x}", ri, gi, bi);
    let rgb_label = format!("rgb({}, {}, {})", ri, gi, bi);

    vec![json!({ "label": hex_label }), json!({ "label": rgb_label })]
}
