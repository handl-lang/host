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

use handl_host_tooling::ToolingHost;
use serde_json::{Value, to_value};

use crate::line_map::LineMap;
use crate::lsp_types::{Location, Position, Range, SymbolInformation};
use crate::refinement::find_arrows_root;

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
pub fn workspace_symbols(
    query: &str,
    workspace_root: Option<&str>,
    host: &ToolingHost,
) -> Vec<Value> {
    let mut results = Vec::new();

    let root = match workspace_root {
        Some(r) => r,
        None => return results,
    };

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
        let lines = LineMap::new(&contents);

        let to_position = |offset: usize| {
            let (line, character) = lines.position(offset);
            Position {
                line: line as u32,
                character: character as u32,
            }
        };
        let to_range = |start: usize, end: usize| Range {
            start: to_position(start),
            end: to_position(end),
        };

        let symbols = match host.document_symbols(&contents) {
            Ok(s) => s,
            Err(_) => continue,
        };

        for s in symbols {
            if !query_lower.is_empty() && !s.name.to_lowercase().contains(&query_lower) {
                continue;
            }

            let info = SymbolInformation {
                name: s.name,
                kind: s.kind,
                location: Location {
                    uri: uri.clone(),
                    range: to_range(s.range_start, s.range_end),
                },
                container_name: container.clone(),
            };
            results.push(to_value(&info).unwrap());
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
