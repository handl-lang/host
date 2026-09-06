//! Call hierarchy support for the handl LSP server.
//!
//! Implements the three LSP call-hierarchy requests:
//!
//! * `textDocument/prepareCallHierarchy` — [`prepare_call_hierarchy`]
//! * `callHierarchy/incomingCalls` — [`incoming_calls`]
//! * `callHierarchy/outgoingCalls` — [`outgoing_calls`]
//!
//! Arrow files (`.hnl`) reference other arrows by name. A reference is either
//! a bare identifier in the arrow body (e.g. `insert -< { ... }` calls the
//! `insert` arrow) or a string literal inside `arr("name")` / `app("name",
//! ...)`. This module lexes arrow files with the canonical kit, locates arrow
//! definitions, and resolves cross-file references by scanning sibling `.hnl`
//! files under the `arrows/` root (located via
//! [`crate::refinement::find_arrows_root`]).
//!
//! ## Arrow definitions
//!
//! A `.hnl` file defines arrows in one of two forms:
//!
//! * **Named** — `name = expr` at the top level (e.g.
//!   `reduce = loop(arr("fold_step") >>> first(id))`). A file may contain
//!   several named arrows, one per line.
//! * **Unnamed** — a single expression (e.g. `loop { ... } ( ... )` or
//!   `loop(body)`). The file stem (the `.hnl` file name without extension) is
//!   the arrow's name.
//!
//! Top-level named-arrow definitions are detected by scanning for an
//! identifier immediately followed by `=` at bracket depth zero, so that
//! let-bindings inside arrow bodies (e.g. `cmp = ... in` within a `loop { ... }
//! (...)` block) are not mistaken for arrow definitions.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use handl_kit_canonical::{
    SpannedToken, is_comment_kind, is_identifier_kind, is_keyword_kind, is_string_kind,
};

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;
use crate::refinement::find_arrows_root;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// LSP `SymbolKind.Function` (value 12), used as the `kind` of every call
/// hierarchy item. Arrows are the handl equivalent of functions.
const KIND_FUNCTION: u32 = 12;

/// A human-readable detail string attached to every call hierarchy item.
const DETAIL: &str = "arrow";

// ---------------------------------------------------------------------------
// Arrow definition model
// ---------------------------------------------------------------------------

/// A single arrow definition located within a source file.
///
/// `name_start` / `name_end` delimit the arrow's name identifier (used as the
/// LSP `selectionRange`). `body_start` / `body_end` delimit the arrow's body
/// expression (the text scanned for outgoing references). `is_file_stem` is
/// `true` for the implicit unnamed arrow whose name is the file stem.
#[derive(Clone, Debug)]
struct ArrowDef {
    name: String,
    name_start: usize,
    name_end: usize,
    body_start: usize,
    body_end: usize,
    is_file_stem: bool,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// `textDocument/prepareCallHierarchy`.
///
/// Returns a single-element list containing a [`CallHierarchyItem`][item] for
/// the arrow at `offset` in `source`, or an empty list when the cursor is not
/// on an arrow name.
///
/// An identifier is considered an arrow name when it is either:
///
/// * the name of a named arrow definition (`name = ...`) in this file, or
/// * the file stem of an unnamed arrow file (the whole file is one arrow).
///
/// [item]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_prepareCallHierarchy
pub fn prepare_call_hierarchy(source: &str, offset: usize, file_path: Option<&str>) -> Vec<Value> {
    let defs = find_arrow_definitions(source, file_path);
    if defs.is_empty() {
        return Vec::new();
    }

    // Resolve the identifier under the cursor.
    let id = identifier_at_offset(source, offset);

    // Case 1: the cursor is on a named arrow's name identifier.
    if let Some((name, _, _)) = &id {
        for def in &defs {
            if !def.is_file_stem && def.name == *name {
                return vec![build_call_hierarchy_item(source, def, file_path)];
            }
        }
    }

    // Case 2: the cursor is on an identifier matching the file stem of an
    // unnamed arrow file.
    if let Some((name, _, _)) = &id {
        for def in &defs {
            if def.is_file_stem && def.name == *name {
                return vec![build_call_hierarchy_item(source, def, file_path)];
            }
        }
    }

    Vec::new()
}

/// `callHierarchy/incomingCalls`.
///
/// Finds every arrow that calls the arrow named `arrow_name`. Scans all `.hnl`
/// files under the `arrows/` root (located from `file_path`), and for each
/// arrow definition checks whether `arrow_name` appears as an identifier (or
/// string literal) in that arrow's body. Returns a list of
/// [`CallHierarchyIncomingCall`][call] objects.
///
/// `file_path` is the file path of the call-hierarchy item the request was
/// made on (its URI with the `file://` prefix stripped). It is used only to
/// locate the `arrows/` root; the target arrow's own file is identified by
/// scanning every `.hnl` file under that root.
///
/// [call]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#callHierarchy_incomingCalls
pub fn incoming_calls(arrow_name: &str, file_path: Option<&str>) -> Vec<Value> {
    let fs_path = file_path.map(to_fs_path);
    let arrows_root = match fs_path.as_deref().and_then(find_arrows_root) {
        Some(root) => root,
        None => return Vec::new(),
    };

    let arrow_files = list_arrow_files(&arrows_root);
    if arrow_files.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (_file_stem, uri) in &arrow_files {
        let disk_path = uri_to_path(uri);
        let contents = match fs::read_to_string(&disk_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let defs = find_arrow_definitions(&contents, Some(&disk_path));
        for def in &defs {
            // An arrow does not "call" itself; skip the target arrow's own
            // definition (which would otherwise show up as a self-call).
            if def.name == arrow_name {
                continue;
            }

            let ranges = find_references_in_body(&contents, def, arrow_name);
            if ranges.is_empty() {
                continue;
            }

            let from = build_call_hierarchy_item(&contents, def, Some(&disk_path));
            out.push(json!({
                "from": from,
                "fromRanges": ranges,
            }));
        }
    }

    out
}

/// `callHierarchy/outgoingCalls`.
///
/// Finds every arrow called by the arrow named `arrow_name`, whose definition
/// lives in the file at `file_path`. Reads that file from disk, locates the
/// arrow definition, scans its body for identifiers (and string literals) that
/// match other `.hnl` arrow file names under the `arrows/` root, and returns a
/// list of [`CallHierarchyOutgoingCall`][call] objects.
///
/// `file_path` is the file path of the call-hierarchy item the request was
/// made on (its URI with the `file://` prefix stripped).
///
/// [call]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#callHierarchy_outgoingCalls
pub fn outgoing_calls(arrow_name: &str, file_path: Option<&str>) -> Vec<Value> {
    let fs_path = file_path.map(to_fs_path);
    let own_path = match fs_path.as_deref() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let arrows_root = match fs_path.as_deref().and_then(find_arrows_root) {
        Some(root) => root,
        None => return Vec::new(),
    };

    // Read the arrow's own file from disk.
    let source = match fs::read_to_string(own_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let defs = find_arrow_definitions(&source, Some(own_path));
    // Locate the arrow named `arrow_name` within the file.
    let current = match defs.iter().find(|d| d.name == arrow_name).cloned() {
        Some(d) => d,
        None => return Vec::new(),
    };

    // Map of arrow name -> file URI for every `.hnl` file under the root.
    let arrow_files = list_arrow_files(&arrows_root);
    if arrow_files.is_empty() {
        return Vec::new();
    }
    let name_to_uri: HashMap<String, String> = arrow_files.iter().cloned().collect();

    // Scan the current arrow's body for references to other arrows.
    let body = &source[current.body_start..current.body_end];
    let body_offset = current.body_start;
    let body_tokens = match lex(body) {
        Some(t) => t,
        None => return Vec::new(),
    };

    // Collect, per referenced arrow, the list of source-relative ranges where
    // the reference occurs. Preserve first-seen order for deterministic
    // output.
    let mut calls: Vec<(String, Vec<Value>)> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for t in &body_tokens {
        if is_comment_kind(&t.kind) {
            continue;
        }

        let ref_name = reference_name(t);
        if ref_name.is_empty() {
            continue;
        }

        // Skip self-references (an arrow calling itself is not an outgoing
        // call to a *different* arrow).
        if ref_name == current.name {
            continue;
        }

        // Only identifiers/strings that name an existing arrow file count.
        if !name_to_uri.contains_key(&ref_name) {
            continue;
        }

        let range = range_from_span(
            &source,
            body_offset + t.span.start,
            body_offset + t.span.end,
        );
        if let Some(&idx) = seen.get(&ref_name) {
            calls[idx].1.push(range);
        } else {
            let idx = calls.len();
            seen.insert(ref_name.clone(), idx);
            calls.push((ref_name, vec![range]));
        }
    }

    // Build the outgoing-call JSON objects.
    let mut out = Vec::with_capacity(calls.len());
    for (name, ranges) in calls {
        let uri = &name_to_uri[&name];
        let to = build_target_call_hierarchy_item(&name, uri);
        out.push(json!({
            "to": to,
            "fromRanges": ranges,
        }));
    }

    out
}

// ---------------------------------------------------------------------------
// Lexing / token helpers
// ---------------------------------------------------------------------------

/// Lex `source` into spanned tokens, returning `None` on failure.
fn lex(source: &str) -> Option<Vec<SpannedToken>> {
    let kit = kit_for_file(None).ok()?;
    kit.lex_with_spans(source).ok()
}

/// Return the identifier at `offset`, as `(name, start, end)`, or `None` when
/// the cursor is not on an identifier.
fn identifier_at_offset(source: &str, offset: usize) -> Option<(String, usize, usize)> {
    let tokens = lex(source)?;
    for t in &tokens {
        if is_identifier_kind(&t.kind) && t.span.start <= offset && offset < t.span.end {
            return Some((t.text.clone(), t.span.start, t.span.end));
        }
    }
    None
}

/// Extract an arrow reference name from a token.
///
/// Identifiers contribute their own text. String literals (used inside
/// `arr("name")` / `app("name", ...)`) contribute their unquoted content.
/// Keywords, operators, and other tokens contribute `None`.
fn reference_name(t: &SpannedToken) -> String {
    if is_identifier_kind(&t.kind) {
        // Keywords are a distinct kind, so identifiers here are never
        // keywords — but guard defensively in case of future grammar
        // changes that fold keywords into the identifier kind.
        if is_keyword_kind(&t.kind) {
            return String::new();
        }
        return t.text.clone();
    }
    if is_string_kind(&t.kind) {
        return unquote_string(&t.text);
    }
    String::new()
}

/// Strip surrounding quotes from a string-literal token's text.
fn unquote_string(text: &str) -> String {
    let mut s = text;
    if s.starts_with('"') {
        s = &s[1..];
    }
    if s.ends_with('"') {
        s = &s[..s.len() - 1];
    }
    s.to_string()
}

// ---------------------------------------------------------------------------
// Arrow definition discovery
// ---------------------------------------------------------------------------

/// Find all arrow definitions in `source`.
///
/// Named arrows (`name = expr` at bracket depth zero) are returned first, in
/// source order. If no named arrows are found, a single unnamed arrow is
/// synthesized whose name is the file stem of `file_path` and whose body is
/// the entire source. Returns an empty vector when there are no named arrows
/// and no `file_path` is available.
fn find_arrow_definitions(source: &str, file_path: Option<&str>) -> Vec<ArrowDef> {
    let tokens = match lex(source) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut defs = Vec::new();
    let mut depth: i32 = 0;
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        let kind = t.kind.as_str();

        if is_comment_kind(kind) {
            i += 1;
            continue;
        }

        // Track bracket depth so that let-bindings inside arrow bodies
        // (`cmp = ... in` within `loop { ... } (...)`) are not mistaken for
        // top-level named-arrow definitions.
        match kind {
            "token.lparen" | "token.lbrace" | "token.lbracket" => depth += 1,
            "token.rparen" | "token.rbrace" | "token.rbracket" => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ => {}
        }

        if depth == 0 && is_identifier_kind(kind) {
            // Look for the next non-comment token; if it is `::`, this is a
            // named-arrow definition.
            let mut j = i + 1;
            while j < tokens.len() && is_comment_kind(&tokens[j].kind) {
                j += 1;
            }
            if j < tokens.len() && tokens[j].text == "::" {
                let body_start = tokens[j].span.end;
                let body_end = line_end_offset(source, tokens[j].span.start);
                defs.push(ArrowDef {
                    name: t.text.clone(),
                    name_start: t.span.start,
                    name_end: t.span.end,
                    body_start,
                    body_end,
                    is_file_stem: false,
                });
                i = j + 1;
                continue;
            }
        }

        i += 1;
    }

    if defs.is_empty() {
        if let Some(stem) = file_path.and_then(file_stem_of) {
            defs.push(ArrowDef {
                name: stem,
                name_start: 0,
                name_end: 0,
                body_start: 0,
                body_end: source.len(),
                is_file_stem: true,
            });
        }
    }

    defs
}

/// Find all source-relative ranges where `name` appears as an arrow reference
/// (identifier or string literal) within `def`'s body, skipping comments.
fn find_references_in_body(source: &str, def: &ArrowDef, name: &str) -> Vec<Value> {
    let body = &source[def.body_start..def.body_end];
    let body_offset = def.body_start;
    let tokens = match lex(body) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut ranges = Vec::new();
    for t in &tokens {
        if is_comment_kind(&t.kind) {
            continue;
        }
        if reference_name(t) == name {
            ranges.push(range_from_span(
                source,
                body_offset + t.span.start,
                body_offset + t.span.end,
            ));
        }
    }
    ranges
}

// ---------------------------------------------------------------------------
// CallHierarchyItem builders
// ---------------------------------------------------------------------------

/// Build a [`CallHierarchyItem`] for an arrow definition in `source`.
///
/// `range` covers the arrow's whole definition (name through body end);
/// `selectionRange` covers just the name identifier. For file-stem (unnamed)
/// arrows, `selectionRange` is a zero-width range at the start of the file.
fn build_call_hierarchy_item(source: &str, def: &ArrowDef, file_path: Option<&str>) -> Value {
    let uri = file_path_to_uri(file_path);
    let range = range_from_span(
        source,
        def.body_start.min(def.name_start),
        def.body_end.max(def.name_end),
    );
    let selection_range = if def.name_end > def.name_start {
        range_from_span(source, def.name_start, def.name_end)
    } else {
        // Unnamed arrow: no name token in the file — point at offset 0.
        range_from_span(source, 0, 0)
    };

    json!({
        "name": def.name,
        "kind": KIND_FUNCTION,
        "uri": uri,
        "range": range,
        "selectionRange": selection_range,
        "detail": DETAIL,
    })
}

/// Build a [`CallHierarchyItem`] for a target arrow in another file.
///
/// Reads the target file from disk to obtain a precise definition range; if
/// the file cannot be read or the arrow cannot be located, falls back to a
/// zero-width range at the start of the file (matching the behaviour of
/// `crate::cross_file_def::find_cross_file_definition_uri`).
fn build_target_call_hierarchy_item(name: &str, uri: &str) -> Value {
    let disk_path = uri_to_path(uri);
    let contents = fs::read_to_string(&disk_path).ok();
    let def = contents.as_deref().and_then(|c| {
        find_arrow_definitions(c, Some(&disk_path))
            .into_iter()
            .find(|d| d.name == name)
    });

    let (range, selection_range) = match (&contents, &def) {
        (Some(c), Some(d)) => {
            let r = range_from_span(
                c,
                d.body_start.min(d.name_start),
                d.body_end.max(d.name_end),
            );
            let sr = if d.name_end > d.name_start {
                range_from_span(c, d.name_start, d.name_end)
            } else {
                range_from_span(c, 0, 0)
            };
            (r, sr)
        }
        _ => (
            json!({
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 },
            }),
            json!({
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 0 },
            }),
        ),
    };

    json!({
        "name": name,
        "kind": KIND_FUNCTION,
        "uri": uri,
        "range": range,
        "selectionRange": selection_range,
        "detail": DETAIL,
    })
}

// ---------------------------------------------------------------------------
// Arrow-file discovery under the arrows root
// ---------------------------------------------------------------------------

/// Recursively list every `.hnl` file under `arrows_root`, returning
/// `(file_stem, file_uri)` pairs.
fn list_arrow_files(arrows_root: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    collect_arrow_files(Path::new(arrows_root), &mut out);
    out
}

/// Recursive worker for [`list_arrow_files`].
fn collect_arrow_files(dir: &Path, out: &mut Vec<(String, String)>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().map(|n| n == "contracts").unwrap_or(false) {
                continue;
            }
            collect_arrow_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("hnl") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                out.push((stem.to_string(), path_to_uri(&path)));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Path / URI helpers
// ---------------------------------------------------------------------------

/// Convert a filesystem path into a `file://` URI.
fn path_to_uri(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    if path_str.starts_with("file://") {
        path_str.into_owned()
    } else if path_str.starts_with('/') {
        format!("file://{}", path_str)
    } else {
        format!("file:///{}", path_str)
    }
}

/// Strip a `file://` prefix from a URI, yielding a filesystem path. Paths that
/// are not URIs are returned unchanged.
fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_string()
}

/// Normalize a `file_path` argument into a filesystem path (stripping any
/// `file://` prefix).
fn to_fs_path(s: &str) -> String {
    s.strip_prefix("file://").unwrap_or(s).to_string()
}

/// Extract the file stem (name without extension) from a file path, returning
/// `None` when the path has no file stem.
fn file_stem_of(file_path: &str) -> Option<String> {
    Path::new(to_fs_path(file_path).as_str())
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Convert an optional `file_path` argument into a `file://` URI. Returns an
/// empty string when no path is available.
fn file_path_to_uri(file_path: Option<&str>) -> String {
    match file_path {
        Some(p) => {
            let fs = to_fs_path(p);
            path_to_uri(Path::new(&fs))
        }
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Range / offset helpers
// ---------------------------------------------------------------------------

/// Convert a byte span in `source` into an LSP `Range` JSON object.
fn range_from_span(source: &str, start: usize, end: usize) -> Value {
    let (start_line, start_char) = offset_to_position(source, start);
    let (end_line, end_char) = offset_to_position(source, end);
    json!({
        "start": { "line": start_line, "character": start_char },
        "end": { "line": end_line, "character": end_char },
    })
}

/// Byte offset of the end of the line containing `from` (the position of the
/// trailing `\n`, or the end of `source`).
fn line_end_offset(source: &str, from: usize) -> usize {
    let bytes = source.as_bytes();
    let mut end = from;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    end
}
