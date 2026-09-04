//! Cross-file goto-definition for arrow files.
//!
//! Arrow files reference other arrows by name (e.g. `insert(...)` in
//! `sort.hnl` references `insert.hnl`). This module resolves those
//! cross-file references into LSP `Location` JSON so that goto-definition
//! can jump from a usage in one `.hnl` file to the definition site in
//! another `.hnl` file under the same `arrows/` root.
//!
//! It also handles axiom definitions: an identifier like `prim.add` is
//! resolved by searching for `axiom prim.add` in `.hnl` files under the
//! `contracts/` directory that is a sibling of the `arrows/` root.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::refinement::find_arrows_root;
use crate::symbols::{find_definition, identifier_at_offset, span_to_range};

/// Given an identifier name and the current file path, search for a `.hnl`
/// file named `name.hnl` in the arrows directory tree.
///
/// Uses `crate::refinement::find_arrows_root` to locate the arrows root
/// from `file_path`, then searches recursively for `name.hnl` under it.
/// Returns the file URI (e.g. `file:///path/to/insert.hnl`) if found.
///
/// For axiom-style identifiers (containing a `.`), this also searches for
/// `axiom <name>` in `.hnl` files under the `contracts/` directory that is
/// a sibling of the arrows root. The axiom search returns the URI of the
/// `.hnl` file containing the axiom declaration.
pub fn find_cross_file_definition(name: &str, file_path: &str) -> Option<String> {
    let arrows_root = find_arrows_root(file_path)?;

    // First, try to find a `.hnl` file named `name.hnl` under the arrows root.
    if let Some(uri) = find_arrow_file_by_name(name, &arrows_root) {
        return Some(uri);
    }

    // If the name contains a dot (e.g. `prim.add`), it may be an axiom
    // reference. Search for `axiom <name>` in `.hnl` files under the
    // sibling `contracts/` directory.
    if name.contains('.') {
        if let Some(uri) = find_axiom_definition(name, &arrows_root) {
            return Some(uri);
        }
    }

    None
}

/// Search recursively for `name.hnl` under `arrows_root`, returning its
/// file URI if found.
fn find_arrow_file_by_name(name: &str, arrows_root: &str) -> Option<String> {
    let target = format!("{}.hnl", name);
    let root = Path::new(arrows_root);
    if !root.is_dir() {
        return None;
    }
    search_for_file(root, &target)
}

/// Recursively search a directory for a file with the given file name,
/// returning its `file://` URI if found.
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

/// Search for `axiom <name>` in `.hnl` files under the `contracts/`
/// directory that is a child of the kit root.
///
/// For example, if `arrows_root` is `.../handl-packages/kit-stdlib/`, the
/// contracts directory is `.../handl-packages/kit-stdlib/contracts/`.
fn find_axiom_definition(name: &str, arrows_root: &str) -> Option<String> {
    let contracts_dir = contracts_dir_from_arrows_root(arrows_root)?;
    let needle = format!("axiom {}", name);
    search_for_text_in_hnl(Path::new(&contracts_dir), &needle)
}

/// Compute the `contracts/` directory path that is a child of the kit root.
///
/// Given `.../handl-packages/kit-stdlib/`, returns
/// `.../handl-packages/kit-stdlib/contracts/`.
fn contracts_dir_from_arrows_root(arrows_root: &str) -> Option<String> {
    let root = Path::new(arrows_root);
    let contracts = root.join("contracts");
    if contracts.is_dir() {
        Some(contracts.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Recursively search `.hnl` files under `dir` for one containing `needle`
/// as a substring, returning the `file://` URI of the first match.
fn search_for_text_in_hnl(dir: &Path, needle: &str) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(uri) = search_for_text_in_hnl(&path, needle) {
                return Some(uri);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("hnl") {
            if let Ok(contents) = fs::read_to_string(&path) {
                if contents.contains(needle) {
                    return Some(path_to_uri(&path));
                }
            }
        }
    }
    None
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

/// Return an LSP `Location` JSON object for the cross-file definition of
/// `name`, resolved from `file_path`.
///
/// The returned JSON has the shape:
/// ```json
/// {
///   "uri": "file:///path/to/insert.hnl",
///   "range": {
///     "start": { "line": 0, "character": 0 },
///     "end": { "line": 0, "character": 0 }
///   }
/// }
/// ```
///
/// The range points at the start of the target file (line 0, character 0)
/// since we do not parse the target file to locate a precise definition
/// site within it.
pub fn find_cross_file_definition_uri(name: &str, file_path: &str) -> Option<Value> {
    let uri = find_cross_file_definition(name, file_path)?;
    Some(json!({
        "uri": uri,
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        }
    }))
}

/// High-level definition resolver.
///
/// Given the source text of the current document, a byte offset within it,
/// and the current file path, this function:
///
/// 1. First tries an in-file definition (using `crate::symbols::find_definition`).
/// 2. If not found, tries a cross-file definition
///    (`find_cross_file_definition_uri`).
///
/// Returns the LSP `Location` JSON for the resolved definition, or `None`
/// if no definition could be found.
pub fn resolve_definition(source: &str, offset: usize, file_path: &str) -> Option<Value> {
    // Step 1: try in-file definition.
    if let Some((name, span)) = identifier_at_offset(source, offset) {
        if let Some(def_span) = find_definition(source, &name) {
            // file_path is already a URI (e.g. "file:///test.hnl") — use as-is.
            let uri = if file_path.starts_with("file://") {
                file_path.to_string()
            } else {
                path_to_uri(&PathBuf::from(file_path))
            };
            return Some(json!({
                "uri": uri,
                "range": span_to_range(source, def_span),
            }));
        }

        // Step 2: fall back to cross-file definition.
        if let Some(loc) = find_cross_file_definition_uri(&name, file_path) {
            return Some(loc);
        }
    }
    None
}
