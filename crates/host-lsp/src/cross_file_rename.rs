//! Cross-file rename for arrow references.
//!
//! Arrow files reference other arrows by name (e.g. `insert(...)` in
//! `sort.hnl` references `insert.hnl`). This module implements LSP
//! `textDocument/prepareRename` and `textDocument/rename` so that renaming
//! an arrow reference updates all occurrences across every `.hnl` file under
//! the `arrows/` root, and optionally renames the arrow file itself.
//!
//! A renameable identifier is one that is:
//! - an identifier token (not a keyword, not a property access after `.`)
//! - either a binding name (followed by `=` or `:`) or an arrow reference
//!   (matches a `.hnl` file name under the arrows root)
//!
//! The returned `WorkspaceEdit` uses `documentChanges` (an array of
//! `TextDocumentEdit` and `RenameFile` resources) so that file renames can
//! be expressed alongside text edits in a single operation.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, is_comment_kind, is_identifier_kind};
use handl_language::Span;

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;
use crate::refinement::find_arrows_root;
use crate::symbols::{find_definition, find_references, span_to_range};

/// Return the current identifier name as an LSP `PrepareRenameResult`, or
/// `None` if the cursor is not on a renameable identifier.
///
/// A renameable identifier is one that:
/// - is an identifier token (not a keyword, not a property access after `.`)
/// - is either a binding name (followed by `=` or `:`) or an arrow reference
///   (matches a `.hnl` file name under the arrows root)
pub fn prepare_rename(source: &str, offset: usize) -> Option<Value> {
    let (name, span) = identifier_at_offset_renameable(source, offset)?;
    let range = span_to_range(source, span);
    Some(json!({
        "range": range,
        "placeholder": name,
    }))
}

/// Return an LSP `WorkspaceEdit` JSON with `TextEdits` across all files that
/// reference the identifier at `offset`, optionally renaming the arrow file
/// itself.
///
/// The edit uses `documentChanges` so that file renames can be expressed
/// alongside text edits. When the identifier matches an arrow file name
/// (e.g. `insert`), all `.hnl` files under the arrows root are searched for
/// references, and a `RenameFile` resource is appended to rename the arrow
/// file (e.g. `insert.hnl` → `newname.hnl`).
pub fn rename(
    source: &str,
    offset: usize,
    new_name: &str,
    file_path: Option<&str>,
) -> Option<Value> {
    let (name, _) = identifier_at_offset_renameable(source, offset)?;

    // Collect text edits for the current file (covers both binding
    // declarations and usages).
    let mut document_changes: Vec<Value> = Vec::new();

    // Current-file edits: all references to `name` in the current source.
    let current_edits: Vec<Value> = find_references(source, &name)
        .into_iter()
        .map(|span| {
            json!({
                "range": span_to_range(source, span),
                "newText": new_name,
            })
        })
        .collect();

    let current_uri = match file_path {
        Some(p) => path_or_uri_to_uri(p),
        None => "file:///untitled.hnl".to_string(),
    };

    if !current_edits.is_empty() {
        document_changes.push(json!({
            "textDocument": { "uri": current_uri, "version": Value::Null },
            "edits": current_edits,
        }));
    }

    // Determine whether `name` matches an arrow file. If so, search all
    // other `.hnl` files under the arrows root for references and add a
    // RenameFile resource.
    let arrow_file_uri = file_path.and_then(|p| find_arrow_file_uri(&name, p));

    if let (Some(fp), Some(arrow_uri)) = (file_path, arrow_file_uri.as_ref()) {
        // Search all .hnl files under the arrows root for references to
        // `name`, excluding the current file (already handled above).
        let arrows_root = find_arrows_root(fp);
        if let Some(root) = arrows_root {
            let root_path = PathBuf::from(&root);
            let current_path = uri_to_path(&current_uri);
            for file_uri in collect_hnl_uris(&root_path) {
                // Skip the current file — already edited above.
                if file_uri == current_uri {
                    continue;
                }
                // Skip the arrow file itself if it equals the current file.
                let file_path_str = uri_to_path(&file_uri);
                if file_path_str == current_path {
                    continue;
                }
                if let Ok(contents) = fs::read_to_string(&file_path_str) {
                    let edits: Vec<Value> = find_references(&contents, &name)
                        .into_iter()
                        .map(|span| {
                            json!({
                                "range": span_to_range(&contents, span),
                                "newText": new_name,
                            })
                        })
                        .collect();
                    if !edits.is_empty() {
                        document_changes.push(json!({
                            "textDocument": { "uri": file_uri, "version": Value::Null },
                            "edits": edits,
                        }));
                    }
                }
            }
        }

        // Append a RenameFile resource to rename the arrow file itself.
        let new_arrow_uri = replace_file_name_in_uri(arrow_uri, new_name);
        document_changes.push(json!({
            "kind": "rename",
            "oldUri": arrow_uri,
            "newUri": new_arrow_uri,
        }));
    }

    if document_changes.is_empty() {
        return None;
    }

    Some(json!({ "documentChanges": document_changes }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the identifier token at `offset` and verify it is renameable.
///
/// Returns `None` if:
/// - there is no identifier at `offset`
/// - the identifier is a property access (preceded by `.`)
/// - the identifier is neither a binding name (followed by `=` or `:`) nor
///   an arrow reference (matches a `.hnl` file name)
fn identifier_at_offset_renameable(source: &str, offset: usize) -> Option<(String, Span)> {
    let kit = kit_for_file(None).ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;

    let (index, token) = tokens.iter().enumerate().find(|(_, t)| {
        is_identifier_kind(t.kind.as_str()) && t.span.start <= offset && offset < t.span.end
    })?;

    // Not a property access (preceded by `.`).
    if index > 0 && tokens[index - 1].kind == "token.dot" {
        return None;
    }

    // Find the next non-comment token to see if this is a binding name.
    let mut j = index + 1;
    while j < tokens.len() && is_comment_kind(&tokens[j].kind) {
        j += 1;
    }
    let is_binding = j < tokens.len()
        && ((tokens[j].kind == "token.bind" && tokens[j].text == "::")
            || tokens[j].kind == "token.colon");

    // Check if it matches an arrow file name. We can't know the file path
    // here (prepare_rename has no path), so we accept identifiers that are
    // either bindings or look like arrow references. For arrow references,
    // any identifier that is followed by `(` is a candidate call.
    let is_arrow_call = j < tokens.len() && tokens[j].kind == "token.lparen";

    if is_binding || is_arrow_call {
        return Some((token.text.clone(), token.span));
    }

    // Also accept a plain identifier that is a usage of a binding (the
    // next significant token is not `=`/`:`/`(`). In that case, treat it as
    // renameable only if a binding with this name exists in the file.
    if find_definition(source, &token.text).is_some() {
        return Some((token.text.clone(), token.span));
    }

    None
}

/// Search for a `.hnl` file named `name.hnl` under the arrows root derived
/// from `file_path`, returning its `file://` URI if found.
fn find_arrow_file_uri(name: &str, file_path: &str) -> Option<String> {
    let root = find_arrows_root(file_path)?;
    let target = format!("{}.hnl", name);
    let root_path = Path::new(&root);
    if !root_path.is_dir() {
        return None;
    }
    search_for_file(root_path, &target)
}

/// Recursively search a directory for a file with the given name, returning
/// its `file://` URI.
fn search_for_file(dir: &Path, file_name: &str) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().map(|n| n == "contracts").unwrap_or(false) {
                continue;
            }
            if let Some(uri) = search_for_file(&path, file_name) {
                return Some(uri);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(file_name) {
            return Some(path_to_uri(&path));
        }
    }
    None
}

/// Recursively collect `file://` URIs for all `.hnl` files under `dir`.
fn collect_hnl_uris(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect_hnl_uris_inner(dir, &mut out);
    out
}

fn collect_hnl_uris_inner(dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().map(|n| n == "contracts").unwrap_or(false) {
                    continue;
                }
                collect_hnl_uris_inner(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("hnl") {
                out.push(path_to_uri(&path));
            }
        }
    }
}

/// Convert a filesystem path or a `file://` URI into a `file://` URI.
fn path_or_uri_to_uri(p: &str) -> String {
    if p.starts_with("file://") {
        p.to_string()
    } else {
        path_to_uri(&PathBuf::from(p))
    }
}

/// Convert a filesystem path into a `file://` URI.
fn path_to_uri(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    if path_str.starts_with('/') {
        format!("file://{}", path_str)
    } else {
        format!("file:///{}", path_str)
    }
}

/// Convert a `file://` URI back into a filesystem path.
fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_string()
}

/// Given a `file://` URI ending in `name.hnl`, produce a new URI ending in
/// `new_name.hnl`.
fn replace_file_name_in_uri(uri: &str, new_name: &str) -> String {
    if let Some(idx) = uri.rfind('/') {
        let dir = &uri[..idx + 1];
        format!("{}{}.hnl", dir, new_name)
    } else {
        uri.to_string()
    }
}

// Silence unused-import warning for SpannedToken when only the type alias is
// used indirectly via `find_references`. The import is kept for clarity and
// future use.
#[allow(dead_code)]
fn _ensure_spanned_token_import(_t: &SpannedToken) {}
