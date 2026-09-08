//! `textDocument/documentLink` support for the handl LSP server.
//!
//! Arrow files reference other arrow files by name in comments, e.g.:
//!
//! ```text
//! -- Written as an arrow graph over insert.
//! --- See: sort.hnl for the sorting arrow
//! ```
//!
//! This module scans comment tokens (both `--` regular comments and `---`
//! doc comments) for three kinds of clickable references and emits LSP
//! [`DocumentLink`][1] JSON objects for each:
//!
//! 1. **File references** — patterns like `foo.hnl` or `bar/baz.hnl`. These
//!    resolve to `file://<arrows_root>/...` URIs so the editor can open the
//!    referenced arrow file.
//! 2. **Arrow references** — patterns like `see insert` or `see: sort`,
//!    which link to the corresponding `<name>.hnl` file under the arrows
//!    root.
//! 3. **URLs** — `http://...` / `https://...`, linked directly.
//!
//! Only comment tokens are scanned; identifiers and other code tokens never
//! produce links. When `file_path` is `None`, file/arrow references cannot
//! be resolved and only URL links are returned.
//!
//! [1]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_documentLink

use std::path::Path;

use serde_json::{Value, json};

use handl_kit_canonical::TokenClasses;

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;
use crate::refinement::find_arrows_root;

/// Compute `textDocument/documentLink` results for `source`.
///
/// Scans every comment token (both `--` and `---` flavours) for file
/// references (`foo.hnl`), arrow references (`see insert`), and URLs
/// (`https://...`). Each match becomes an LSP `DocumentLink` JSON object:
///
/// ```json
/// {
///   "range": { "start": {"line": L, "character": C1},
///              "end":   {"line": L, "character": C2} },
///   "target": "file:///path/to/target.hnl",
///   "tooltip": "Open insert.hnl"
/// }
/// ```
///
/// File and arrow references are resolved against the arrows root derived
/// from `file_path` (see [`find_arrows_root`]). When `file_path` is `None`,
/// only URL links are returned — file references cannot be resolved without
/// a path.
pub fn document_links(source: &str, file_path: Option<&str>) -> Vec<Value> {
    let kit = match kit_for_file(file_path) {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let classes = TokenClasses::new(kit.language());
    let mut links = Vec::new();
    for token in &tokens {
        if !classes.is_comment(&token.kind) {
            continue;
        }
        let comment_start = token.span.start;
        // The comment text is a subslice of `source`, so byte offsets within
        // it map directly to absolute source offsets by adding `comment_start`.
        let comment_text = &source[token.span.start..token.span.end];
        links.extend(links_in_comment(
            source,
            comment_text,
            comment_start,
            file_path,
        ));
    }
    links
}

/// Find all clickable links within a single comment token.
///
/// `comment_text` is the comment's source slice and `comment_start` is its
/// absolute byte offset in `source`. Matches are reported with absolute
/// offsets so that [`offset_to_position`] can convert them to LSP positions.
fn links_in_comment(
    source: &str,
    comment_text: &str,
    comment_start: usize,
    file_path: Option<&str>,
) -> Vec<Value> {
    let mut out = Vec::new();

    // URLs are collected first so that file/arrow reference scanning can
    // skip matches that fall inside a URL.
    let url_matches = find_urls(comment_text);
    let url_spans: Vec<(usize, usize)> = url_matches.iter().map(|(s, e, _)| (*s, *e)).collect();

    for (rel_start, rel_end, url) in &url_matches {
        out.push(make_link(
            source,
            comment_start + rel_start,
            comment_start + rel_end,
            url.clone(),
            url.clone(),
        ));
    }

    // File references (`foo.hnl`, `bar/baz.hnl`).
    let file_matches = find_file_refs(comment_text, &url_spans);
    let file_spans: Vec<(usize, usize)> = file_matches.iter().map(|(s, e, _)| (*s, *e)).collect();

    for (rel_start, rel_end, file_ref) in &file_matches {
        let tooltip = format!("Open {}", file_ref);
        if let Some(target) = resolve_file_target(file_ref, file_path) {
            out.push(make_link(
                source,
                comment_start + rel_start,
                comment_start + rel_end,
                target,
                tooltip,
            ));
        }
    }

    // Arrow references (`see insert`, `see: sort`). Skip any that overlap a
    // file reference (e.g. `see sort.hnl` already yields a file link).
    let arrow_matches = find_arrow_refs(comment_text, &url_spans, &file_spans);
    for (rel_start, rel_end, name) in &arrow_matches {
        let tooltip = format!("Open {}.hnl", name);
        if let Some(target) = resolve_file_target(&format!("{}.hnl", name), file_path) {
            out.push(make_link(
                source,
                comment_start + rel_start,
                comment_start + rel_end,
                target,
                tooltip,
            ));
        }
    }

    out
}

/// Build a single `DocumentLink` JSON object.
fn make_link(
    source: &str,
    abs_start: usize,
    abs_end: usize,
    target: String,
    tooltip: String,
) -> Value {
    let (start_line, start_char) = offset_to_position(source, abs_start);
    let (end_line, end_char) = offset_to_position(source, abs_end);
    json!({
        "range": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": end_line, "character": end_char },
        },
        "target": target,
        "tooltip": tooltip,
    })
}

// ---------------------------------------------------------------------------
// Pattern scanning
// ---------------------------------------------------------------------------

/// Find `http://` / `https://` URLs in `text`.
///
/// Returns `(start, end, url)` tuples with byte offsets relative to `text`.
/// A URL runs from its scheme until the first whitespace, quote, or end of
/// text, with trailing punctuation (`.`, `,`, `;`, `)`, `]`) trimmed.
fn find_urls(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut search_from = 0;
    loop {
        let rest = &text[search_from..];
        let http = rest.find("http://");
        let https = rest.find("https://");
        let pos = match (http, https) {
            (Some(h), Some(hs)) => Some(search_from + h.min(hs)),
            (Some(h), None) => Some(search_from + h),
            (None, Some(hs)) => Some(search_from + hs),
            (None, None) => None,
        };
        let start = match pos {
            Some(s) => s,
            None => break,
        };

        // Extend to the first whitespace, quote, or end of text.
        let mut end = start;
        for (i, ch) in text[start..].char_indices() {
            if ch.is_whitespace() || ch == '"' || ch == '\'' {
                end = start + i;
                break;
            }
            end = start + i + ch.len_utf8();
        }

        // Trim trailing punctuation that is unlikely to be part of the URL.
        while end > start {
            let last = text[start..end].chars().last().unwrap();
            if matches!(last, '.' | ',' | ';' | ')' | ']') {
                end -= last.len_utf8();
            } else {
                break;
            }
        }

        out.push((start, end, text[start..end].to_string()));
        search_from = end.max(start + 1);
    }
    out
}

/// Find file references like `foo.hnl` or `bar/baz.hnl` in `text`.
///
/// Returns `(start, end, reference)` tuples with byte offsets relative to
/// `text`. Matches that begin inside one of `url_spans` are skipped so that
/// the `.hnl` suffix of a URL path is not treated as a file reference.
fn find_file_refs(text: &str, url_spans: &[(usize, usize)]) -> Vec<(usize, usize, String)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if is_path_char(bytes[i]) {
            let start = i;
            while i < bytes.len() && is_path_char(bytes[i]) {
                i += 1;
            }
            if text[i..].starts_with(".hnl") {
                let end = i + 4;
                if !inside_any(start, url_spans) {
                    out.push((start, end, text[start..end].to_string()));
                }
                i = end;
            }
            // Otherwise `i` already sits at the first non-path byte; the
            // outer loop continues from there.
        } else {
            i += 1;
        }
    }
    out
}

/// Find arrow references like `see insert` or `see: sort` in `text`.
///
/// Returns `(start, end, name)` tuples where `name` is the referenced arrow
/// identifier (e.g. `insert`) and the offsets cover just the identifier.
/// Matches whose identifier overlaps a URL span or a file-reference span
/// are skipped (e.g. `see sort.hnl` only yields the file reference).
fn find_arrow_refs(
    text: &str,
    url_spans: &[(usize, usize)],
    file_spans: &[(usize, usize)],
) -> Vec<(usize, usize, String)> {
    let bytes = text.as_bytes();
    let lower: Vec<u8> = text.bytes().map(|b| b.to_ascii_lowercase()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 <= lower.len() {
        if &lower[i..i + 3] == b"see" {
            let prev_ok = i == 0 || !(lower[i - 1].is_ascii_alphanumeric() || lower[i - 1] == b'_');
            let after = i + 3;
            let next_ok = after >= lower.len()
                || !(lower[after].is_ascii_alphanumeric() || lower[after] == b'_');
            if prev_ok && next_ok {
                let mut p = after;
                if p < bytes.len() && bytes[p] == b':' {
                    p += 1;
                }
                while p < bytes.len() && (bytes[p] as char).is_whitespace() {
                    p += 1;
                }
                if p < bytes.len() && (bytes[p].is_ascii_alphabetic() || bytes[p] == b'_') {
                    let name_start = p;
                    while p < bytes.len() && (bytes[p].is_ascii_alphanumeric() || bytes[p] == b'_')
                    {
                        p += 1;
                    }
                    if !inside_any(name_start, url_spans)
                        && !inside_any(name_start, file_spans)
                        && !overlaps_any(name_start, p, file_spans)
                    {
                        out.push((name_start, p, text[name_start..p].to_string()));
                    }
                    i = p;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

/// Resolve a file reference (e.g. `sort.hnl` or `collection/insert.hnl`) to
/// a `file://` URI.
///
/// Bare filenames (`sort.hnl`) resolve relative to the current file's
/// directory. Paths containing a separator (`collection/insert.hnl`) resolve
/// relative to the arrows root. Returns `None` when `file_path` is `None` or
/// no arrows root can be derived from it.
fn resolve_file_target(file_ref: &str, file_path: Option<&str>) -> Option<String> {
    let file_path = file_path?;
    let fs_path = file_path.strip_prefix("file://").unwrap_or(file_path);
    let arrows_root = find_arrows_root(fs_path)?;

    let target = if file_ref.contains('/') {
        // Relative to the arrows root.
        let root = arrows_root.trim_end_matches('/');
        format!("{}/{}", root, file_ref)
    } else {
        // Relative to the current file's directory.
        let dir = Path::new(fs_path).parent()?;
        let dir = dir.to_string_lossy().trim_end_matches('/').to_string();
        format!("{}/{}", dir, file_ref)
    };

    Some(path_to_uri(&target))
}

/// Convert a filesystem path string into a `file://` URI.
fn path_to_uri(path: &str) -> String {
    if path.starts_with('/') {
        format!("file://{}", path)
    } else {
        format!("file:///{}", path)
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Whether `b` is a valid character within a file-reference path component.
fn is_path_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'/' || b == b'-'
}

/// Whether byte offset `pos` falls within any of the `(start, end)` spans.
fn inside_any(pos: usize, spans: &[(usize, usize)]) -> bool {
    spans.iter().any(|&(s, e)| pos >= s && pos < e)
}

/// Whether the range `[start, end)` overlaps any of the given spans.
fn overlaps_any(start: usize, end: usize, spans: &[(usize, usize)]) -> bool {
    spans.iter().any(|&(s, e)| start < e && end > s)
}

// `TokenClasses::is_comment` covers both comment flavours (`comment` and
// `doc-comment` categories) — doc comments are link-scannable too.
