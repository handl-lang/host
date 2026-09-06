//! `workspace/symbol` support for the handl LSP server.
//!
//! This module implements the `workspace/symbol` request so that VSCode's
//! "Go to Symbol in Workspace" (`Ctrl+T`) can search for arrows and axioms
//! across all `.hnl` files under the kit root and the contracts directory.
//!
//! Each matching symbol is returned as a
//! [`SymbolInformation`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#workspace_symbol)
//! JSON object, with `containerName` set to the parent directory of the
//! owning file (e.g. `collection`, `fn`, `parse`, `state`, or `contracts`).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::document_symbols::document_symbols;
use crate::refinement::find_arrows_root;

// `offset_to_position` is re-exported here so callers translating raw byte
// spans (e.g. from custom lexers) can reuse the same position conversion the
// rest of the crate uses. It is not needed internally because
// `document_symbols` already emits line/character ranges.
#[allow(unused_imports)]
pub use crate::diagnostics::offset_to_position;

/// Recursively collect every `.hnl` file under `dir`.
fn collect_hnl_files(dir: &Path, out: &mut Vec<PathBuf>) {
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
            collect_hnl_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("hnl") {
            out.push(path);
        }
    }
}

/// Convert a byte offset span (from a `document_symbols` entry) into an LSP
/// range JSON object. The document symbols store `range`/`selectionRange` as
/// already-computed `{start,end}` objects, so we simply reuse those.
fn range_from_symbol(symbol: &Value) -> Value {
    // Prefer the full `range`; fall back to `selectionRange`.
    if let Some(range) = symbol.get("range") {
        return range.clone();
    }
    if let Some(sel) = symbol.get("selectionRange") {
        return sel.clone();
    }
    // Should never happen, but produce a sane default.
    json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 0 },
    })
}

/// Build a `file://` URI from an absolute path.
fn path_to_uri(path: &Path) -> String {
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    format!("file://{}", abs.to_string_lossy())
}

/// Search all `.hnl` files under the kit root (and the child contracts
/// directory) for symbols whose name contains `query` (case-insensitive).
///
/// When `query` is empty, every symbol is returned — VSCode shows the full
/// list when `Ctrl+T` is pressed without typing a filter.
///
/// Results are sorted alphabetically by symbol name.
pub fn workspace_symbols(query: &str, workspace_root: Option<&str>) -> Vec<Value> {
    let mut results = Vec::new();

    let root = match workspace_root {
        Some(r) => r,
        None => return results,
    };

    let root_path = Path::new(root);

    // Locate the kit root. `find_arrows_root` looks for
    // `/handl-packages/<pkg>/` within the path string.
    let arrows_root: Option<PathBuf> = find_arrows_root(root).map(PathBuf::from);

    // Locate the contracts directory. It is a child of the kit root
    // (`.../handl-packages/contracts/`).
    let contracts_dir: Option<PathBuf> = arrows_root
        .as_ref()
        .map(|ar| ar.join("contracts"))
        .filter(|p| p.is_dir());

    // Gather candidate files from both roots.
    let mut files = Vec::new();
    if let Some(ref ar) = arrows_root {
        collect_hnl_files(ar, &mut files);
    }
    if let Some(ref cd) = contracts_dir {
        collect_hnl_files(cd, &mut files);
    }

    let query_lower = query.to_lowercase();

    for file in &files {
        let contents = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // `containerName` is the parent directory name of the file.
        let container = file
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let uri = path_to_uri(file);

        for symbol in document_symbols(&contents) {
            let name = symbol
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();

            if !query_lower.is_empty() && !name.to_lowercase().contains(&query_lower) {
                continue;
            }

            let kind = symbol.get("kind").and_then(|k| k.as_u64()).unwrap_or(13) as u32;

            let range = range_from_symbol(&symbol);

            results.push(json!({
                "name": name,
                "kind": kind,
                "location": {
                    "uri": uri,
                    "range": range,
                },
                "containerName": container,
            }));
        }
    }

    // Sort alphabetically by name (stable enough for display purposes).
    results.sort_by(|a, b| {
        let an = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let bn = b.get("name").and_then(|n| n.as_str()).unwrap_or("");
        an.cmp(bn)
    });

    results
}
