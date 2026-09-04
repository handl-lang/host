//! Type hierarchy support for the handl LSP server.
//!
//! Implements the three LSP type-hierarchy requests:
//!
//! * `textDocument/prepareTypeHierarchy` — [`prepare_type_hierarchy`]
//! * `typeHierarchy/supertypes` — [`supertypes`]
//! * `typeHierarchy/subtypes` — [`subtypes`]
//!
//! The handl refinement type system defines *predicates* (`Sorted`,
//! `LengthEq`, `AllSatisfy`, `HeadOf`, `IsPerm`, …) in contract files under
//! `contracts/` via `axiom <name>` declarations. Arrow files (`.hnl` under
//! `arrows/`) have inferred guarantees that reference these predicates.
//!
//! A *supertype* of a predicate is a more general predicate it implies (e.g.
//! `Sorted` implies `IsPerm`, so `IsPerm` is a supertype of `Sorted`). A
//! *subtype* is more specific (e.g. `Sorted` is a subtype of `IsPerm`).
//! Additionally, any arrow whose inferred guarantee references a predicate is
//! treated as a *subtype* (a concrete implementation satisfying that
//! predicate).
//!
//! The known implication relationships are hardcoded from the stdlib
//! contracts. Cross-file predicate locations are resolved by scanning the
//! `contracts/` directory (a sibling of the `arrows/` root, located via
//! [`crate::refinement::find_arrows_root`]); arrow implementations are
//! discovered by scanning `.hnl` files under the `arrows/` root and running
//! the refinement engine on each.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, is_identifier_kind, is_keyword_kind};

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;
use crate::refinement::{RefinementEngine, find_arrows_root};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// LSP `SymbolKind.TypeParameter` (value 5), used as the `kind` of every type
/// hierarchy item. Refinement predicates are the handl equivalent of type
/// parameters/constructors in the type hierarchy.
const KIND_TYPE_PARAMETER: u32 = 5;

/// Known type predicate names from the refinement type system.
///
/// These are the predicate constructors defined in `Pred` (see
/// `plugin-ext-refinement/src/types.rs`). When the cursor is on one of these
/// names, it is treated as a type-hierarchy subject.
const TYPE_PREDICATES: &[&str] = &[
    "Sorted",
    "LengthEq",
    "LengthLe",
    "AllSatisfy",
    "Le",
    "HeadOf",
    "LengthPlusOne",
    "LengthSubOne",
    "IsPerm",
    "IsSuccess",
    "IsError",
    "Forall",
];

/// Hardcoded supertype relationships from the stdlib contracts.
///
/// `Sorted` implies `IsPerm` (a sorted list is a permutation of itself), so
/// `IsPerm` is a supertype of `Sorted`. Base predicates like `LengthEq` and
/// `AllSatisfy` have no supertypes.
fn supertypes_of(name: &str) -> Vec<&'static str> {
    match name {
        "Sorted" => vec!["IsPerm"],
        _ => Vec::new(),
    }
}

/// Hardcoded subtype relationships from the stdlib contracts.
///
/// `IsPerm` has `Sorted` as a subtype (a sorted list is a specific kind of
/// permutation). Other predicates have no hardcoded subtypes (though arrow
/// implementations may still be discovered dynamically).
fn subtypes_of(name: &str) -> Vec<&'static str> {
    match name {
        "IsPerm" => vec!["Sorted"],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// `textDocument/prepareTypeHierarchy`.
///
/// Returns a single-element list containing a [`TypeHierarchyItem`][item] for
/// the type/predicate at `offset` in `source`, or an empty list when the
/// cursor is not on a type-hierarchy subject.
///
/// An identifier is a type-hierarchy subject when it is either:
///
/// * a known type predicate name (e.g. `Sorted`, `LengthEq`), or
/// * an arrow name whose inferred guarantee references a predicate (the item
///   represents that guarantee predicate).
///
/// [item]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_prepareTypeHierarchy
pub fn prepare_type_hierarchy(
    source: &str,
    offset: usize,
    file_path: Option<&str>,
) -> Vec<Value> {
    let (name, start, end) = match identifier_at_offset(source, offset) {
        Some(id) => id,
        None => return Vec::new(),
    };

    // Case 1: the cursor is on a known type predicate name.
    if is_type_predicate_name(&name) {
        return vec![predicate_item_from_source(
            source,
            file_path,
            &name,
            start,
            end,
            "predicate",
        )];
    }

    // Case 2: the cursor is on an arrow name — infer the document and return
    // an item for the inferred guarantee predicate.
    let engine = RefinementEngine::new();
    if let Some(Ok(result)) = engine.infer_document(source, file_path) {
        let guarantee_str = result.guarantee.simplify().pretty();
        if let Some(predicate) = extract_predicate_name(&guarantee_str) {
            // Point the item at the predicate's axiom definition in contract
            // files when available; otherwise fall back to the arrow name
            // span in the current document.
            if let Some(item) = predicate_item_from_contracts(file_path, &predicate) {
                return vec![item];
            }
            return vec![predicate_item_from_source(
                source,
                file_path,
                &predicate,
                start,
                end,
                "guarantee predicate",
            )];
        }
    }

    Vec::new()
}

/// `typeHierarchy/supertypes`.
///
/// Returns the supertypes of the type/predicate at `offset` in `source`.
///
/// For a predicate like `Sorted`, this returns the more general predicates it
/// implies (e.g. `IsPerm`). Base predicates like `LengthEq` and `AllSatisfy`
/// have no supertypes and produce an empty list.
///
/// [item]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#typeHierarchy_supertypes
pub fn supertypes(
    source: &str,
    offset: usize,
    file_path: Option<&str>,
) -> Vec<Value> {
    let (name, _start, _end) = match identifier_at_offset(source, offset) {
        Some(id) => id,
        None => return Vec::new(),
    };

    if !is_type_predicate_name(&name) {
        return Vec::new();
    }

    let supers = supertypes_of(&name);
    let mut out = Vec::with_capacity(supers.len());
    for sup in supers {
        if let Some(item) = predicate_item_from_contracts(file_path, sup) {
            out.push(item);
        } else {
            // Fall back to a minimal item pointing at the current document
            // when the predicate's axiom cannot be located in contracts.
            out.push(fallback_predicate_item(file_path, sup));
        }
    }
    out
}

/// `typeHierarchy/subtypes`.
///
/// Returns the subtypes of the type/predicate at `offset` in `source`.
///
/// For a predicate like `IsPerm`, this returns more specific predicates (e.g.
/// `Sorted`). Additionally, any arrow file under the `arrows/` root whose
/// inferred guarantee references the predicate is treated as a subtype (a
/// concrete implementation satisfying the predicate).
///
/// [item]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#typeHierarchy_subtypes
pub fn subtypes(
    source: &str,
    offset: usize,
    file_path: Option<&str>,
) -> Vec<Value> {
    let (name, _start, _end) = match identifier_at_offset(source, offset) {
        Some(id) => id,
        None => return Vec::new(),
    };

    if !is_type_predicate_name(&name) {
        return Vec::new();
    }

    let mut out = Vec::new();

    // Hardcoded predicate subtypes (e.g. IsPerm → Sorted).
    for sub in subtypes_of(&name) {
        if let Some(item) = predicate_item_from_contracts(file_path, sub) {
            out.push(item);
        } else {
            out.push(fallback_predicate_item(file_path, sub));
        }
    }

    // Dynamic arrow implementations: any arrow whose inferred guarantee
    // references the predicate is a subtype (an implementation).
    if let Some(impls) = arrow_implementations_of(file_path, &name) {
        out.extend(impls);
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
/// the cursor is not on an identifier (or is on a keyword).
fn identifier_at_offset(source: &str, offset: usize) -> Option<(String, usize, usize)> {
    let tokens = lex(source)?;
    for t in &tokens {
        if is_identifier_kind(&t.kind)
            && !is_keyword_kind(&t.kind)
            && t.span.start <= offset
            && offset < t.span.end
        {
            return Some((t.text.clone(), t.span.start, t.span.end));
        }
    }
    None
}

/// Check if a name is a known type predicate from the refinement type system.
fn is_type_predicate_name(name: &str) -> bool {
    TYPE_PREDICATES.iter().any(|p| *p == name)
}

/// Extract the first known type predicate name appearing in a pretty-printed
/// predicate/type string. Returns `None` if no known predicate appears.
fn extract_predicate_name(type_str: &str) -> Option<String> {
    for pred in TYPE_PREDICATES {
        if type_str.contains(pred) {
            return Some((*pred).to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// TypeHierarchyItem builders
// ---------------------------------------------------------------------------

/// Build a [`TypeHierarchyItem`] for a predicate, anchored at the byte span
/// `[start, end)` in the in-memory `source`.
///
/// `range` and `selectionRange` both cover `[start, end)`, computed against
/// `source`. When no `file_path` is available, `uri` is an empty string.
fn predicate_item_from_source(
    source: &str,
    file_path: Option<&str>,
    name: &str,
    start: usize,
    end: usize,
    detail: &str,
) -> Value {
    let uri = file_path_to_uri(file_path);
    let range = range_from_source(source, start, end);
    json!({
        "name": name,
        "kind": KIND_TYPE_PARAMETER,
        "uri": uri,
        "range": range,
        "selectionRange": range,
        "detail": detail,
    })
}

/// Build a minimal fallback [`TypeHierarchyItem`] for a predicate whose axiom
/// could not be located in contract files. The item points at the current
/// document with a zero-width range at the origin.
fn fallback_predicate_item(file_path: Option<&str>, name: &str) -> Value {
    let uri = file_path_to_uri(file_path);
    let range = json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 0 },
    });
    json!({
        "name": name,
        "kind": KIND_TYPE_PARAMETER,
        "uri": uri,
        "range": range,
        "selectionRange": range,
        "detail": "predicate",
    })
}

/// Build a [`TypeHierarchyItem`] for a predicate located by searching contract
/// files for its `axiom <name>` declaration.
///
/// Returns `None` when the predicate's axiom cannot be located (no `contracts/`
/// directory, or the name does not appear in any contract file). The returned
/// item's `range` and `selectionRange` cover just the predicate name within
/// the `axiom <name>` line.
fn predicate_item_from_contracts(file_path: Option<&str>, name: &str) -> Option<Value> {
    let (uri, line, char, len) = find_predicate_in_contracts(file_path, name)?;
    let range = json!({
        "start": { "line": line, "character": char },
        "end": { "line": line, "character": char + len },
    });
    Some(json!({
        "name": name,
        "kind": KIND_TYPE_PARAMETER,
        "uri": uri,
        "range": range,
        "selectionRange": range,
        "detail": "predicate",
    }))
}

/// Build a [`TypeHierarchyItem`] for an arrow implementation (a subtype
/// discovered by scanning arrow files).
fn arrow_implementation_item(name: &str, uri: &str) -> Value {
    let range = json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 0 },
    });
    json!({
        "name": name,
        "kind": KIND_TYPE_PARAMETER,
        "uri": uri,
        "range": range,
        "selectionRange": range,
        "detail": "arrow implementation",
    })
}

// ---------------------------------------------------------------------------
// Contract-file search
// ---------------------------------------------------------------------------

/// Search for a predicate's `axiom <name>` declaration in contract files under
/// the `contracts/` directory (a sibling of the `arrows/` root).
///
/// Returns `(uri, line, character, name_len)` pointing at the predicate name
/// within the `axiom <name>` line, or `None` if not found.
fn find_predicate_in_contracts(
    file_path: Option<&str>,
    name: &str,
) -> Option<(String, u32, u32, u32)> {
    let path = file_path?;
    let arrows_root = find_arrows_root(path)?;
    let contracts_dir = contracts_dir_from_arrows_root(&arrows_root)?;
    search_axiom_in_dir(Path::new(&contracts_dir), name)
}

/// Recursively search `.hnl` files under `dir` for an `axiom <name>`
/// declaration, returning `(uri, line, character, name_len)` for the first
/// match. The character offset points at the name (after `axiom `).
fn search_axiom_in_dir(dir: &Path, name: &str) -> Option<(String, u32, u32, u32)> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = search_axiom_in_dir(&path, name) {
                return Some(found);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("hnl") {
            if let Ok(contents) = fs::read_to_string(&path) {
                let needle = format!("axiom {}", name);
                if let Some(byte_offset) = contents.find(&needle) {
                    let uri = path_to_uri(&path);
                    let (line, character) = offset_to_position(&contents, byte_offset);
                    // The name begins after `axiom ` (6 characters).
                    let name_char = character + "axiom ".len() as u32;
                    let name_len = name.len() as u32;
                    return Some((uri, line, name_char, name_len));
                }
            }
        }
    }
    None
}

/// Compute the `contracts/` directory path that is a child of the kit root.
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

// ---------------------------------------------------------------------------
// Arrow implementation discovery
// ---------------------------------------------------------------------------

/// Find every arrow file under the `arrows/` root whose inferred guarantee
/// references `predicate`. Returns a list of [`TypeHierarchyItem`] objects
/// (one per matching arrow), or `None` when the arrows root cannot be located.
fn arrow_implementations_of(file_path: Option<&str>, predicate: &str) -> Option<Vec<Value>> {
    let path = file_path?;
    let arrows_root = find_arrows_root(path)?;
    let engine = RefinementEngine::new();

    let mut out = Vec::new();
    let mut visited = std::collections::HashSet::new();
    collect_arrow_implementations(
        Path::new(&arrows_root),
        predicate,
        &engine,
        &mut out,
        &mut visited,
    );
    Some(out)
}

/// Recursive worker for [`arrow_implementations_of`].
fn collect_arrow_implementations(
    dir: &Path,
    predicate: &str,
    engine: &RefinementEngine,
    out: &mut Vec<Value>,
    visited: &mut std::collections::HashSet<String>,
) {
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
            collect_arrow_implementations(&path, predicate, engine, out, visited);
        } else if path.extension().and_then(|e| e.to_str()) == Some("hnl") {
            let uri = path_to_uri(&path);
            if !visited.insert(uri.clone()) {
                continue;
            }
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let contents = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let path_str = path.to_string_lossy().to_string();
            if let Some(Ok(result)) = engine.infer_document(&contents, Some(&path_str)) {
                let guarantee_str = result.guarantee.simplify().pretty();
                if guarantee_str.contains(predicate) {
                    out.push(arrow_implementation_item(&stem, &uri));
                }
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
fn to_fs_path(s: &str) -> String {
    s.strip_prefix("file://").unwrap_or(s).to_string()
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
fn range_from_source(source: &str, start: usize, end: usize) -> Value {
    let (start_line, start_char) = offset_to_position(source, start);
    let (end_line, end_char) = offset_to_position(source, end);
    json!({
        "start": { "line": start_line, "character": start_char },
        "end": { "line": end_line, "character": end_char },
    })
}
