//! `textDocument/typeDefinition` support for the handl LSP.
//!
//! Jumps to the inferred refinement type of a symbol. For property accesses
//! (e.g. `state.acc`), the refinement engine infers the property's type and
//! we locate where that type's predicates are defined in contract files. For
//! arrow names, we infer the document's input/output types and search for
//! their predicate definitions. For type predicate names (e.g. `Sorted`,
//! `LengthEq`), we search for their axiom definitions in contract files.
//!
//! Contract files (`.hnl` under `contracts/`) define type predicates via
//! `axiom <name>` declarations. The search uses `find_arrows_root` to locate
//! the arrows root from the document's file path, then looks for the sibling
//! `contracts/` directory.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, TokenClasses};
use handl_plugin_ext_refinement::RefTy;

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;
use crate::refinement::{RefinementEngine, find_arrows_root};

/// Known type predicate names from the refinement type system.
///
/// These are the predicate constructors defined in `Pred` (see
/// `plugin-ext-refinement/src/types.rs`). When the cursor is on one of
/// these names, we search contract files for its axiom definition.
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

/// Entry point for `textDocument/typeDefinition`.
///
/// Given the source text of a document, a byte offset within it, an optional
/// file path (filesystem path, not URI), and the refinement engine, this
/// returns an LSP `Location` JSON object pointing to where the type of the
/// symbol at the cursor is defined.
///
/// The returned JSON has the shape:
/// ```json
/// {
///   "uri": "file:///path/to/contracts/list.hnl",
///   "range": {
///     "start": { "line": 0, "character": 0 },
///     "end":   { "line": 0, "character": 6 }
///   }
/// }
/// ```
///
/// Returns `None` if no type definition can be found.
pub fn type_definition(
    source: &str,
    offset: usize,
    file_path: Option<&str>,
    refinement: &RefinementEngine,
) -> Option<Value> {
    // Lex the source and find the token at the cursor position.
    let kit = kit_for_file(file_path).ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());

    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.span.start <= offset && offset < t.span.end)?;

    // The cursor must be on an identifier.
    if !classes.is_identifier(token.kind.as_str()) {
        return None;
    }

    let name = token.text.clone();

    // Case 1: Property access (e.g. `state.acc`).
    // The token is preceded by `.` — infer the property's refinement type
    // and search for where its predicates are defined.
    if index > 0 && classes.is_label(&tokens[index - 1].kind, ".") {
        return type_definition_for_property(source, file_path, refinement, &name);
    }

    // Case 2: Type predicate name (e.g. `Sorted`, `LengthEq`).
    // Search directly for the axiom definition in contract files.
    if is_type_predicate_name(&name) {
        return search_in_contracts(file_path, &name);
    }

    // Case 3: Arrow name — infer the document's input/output types and
    // search for where their predicates are defined. Falls back to
    // searching for the arrow name itself as an axiom.
    type_definition_for_arrow(source, file_path, refinement, &name)
}

/// Check if a name is a known type predicate from the refinement type system.
fn is_type_predicate_name(name: &str) -> bool {
    TYPE_PREDICATES.iter().any(|p| *p == name)
}

/// Type definition for a property access (e.g. `state.acc`).
///
/// Uses the refinement engine to infer the type of the property, then
/// searches for where that type's predicates are defined in contract files.
fn type_definition_for_property(
    source: &str,
    file_path: Option<&str>,
    refinement: &RefinementEngine,
    property_name: &str,
) -> Option<Value> {
    // Infer the document's refinement types.
    let result = refinement.infer_document(source, file_path)?.ok()?;

    // Parse the rec state declaration to get field names and positions.
    let fields = parse_rec_state_fields(source)?;
    let pos = fields.iter().position(|f| f == property_name)?;

    // Extract the component types at that position from input and output.
    let input_component = extract_product_component(&result.input, pos);
    let output_component = extract_product_component(&result.output, pos);

    // Search for predicates in the component types.
    for component in [input_component, output_component].into_iter().flatten() {
        let type_str = component.pretty();
        if let Some(predicate) = extract_predicate_name(&type_str) {
            if let Some(loc) = search_in_contracts(file_path, &predicate) {
                return Some(loc);
            }
        }
    }

    None
}

/// Type definition for an arrow name.
///
/// Infers the document's input/output types and searches for where their
/// predicates are defined in contract files. Falls back to searching for
/// the arrow name itself as an axiom definition.
fn type_definition_for_arrow(
    source: &str,
    file_path: Option<&str>,
    refinement: &RefinementEngine,
    name: &str,
) -> Option<Value> {
    // Try to infer the document and extract predicates from the types.
    if let Some(Ok(result)) = refinement.infer_document(source, file_path) {
        let input_str = result.input.pretty();
        let output_str = result.output.pretty();
        let guarantee_str = result.guarantee.simplify().pretty();

        // Search for predicates in input, output, and guarantee types.
        for type_str in [&input_str, &output_str, &guarantee_str] {
            if let Some(predicate) = extract_predicate_name(type_str) {
                if let Some(loc) = search_in_contracts(file_path, &predicate) {
                    return Some(loc);
                }
            }
        }
    }

    // Fall back to searching for the arrow name as an axiom in contracts.
    search_in_contracts(file_path, name)
}

/// Extract a type predicate name from a pretty-printed type or predicate
/// string.
///
/// Scans the string for known predicate names (e.g. `Sorted`, `AllSatisfy`).
/// Returns the first match found, or `None` if no known predicate appears.
fn extract_predicate_name(type_str: &str) -> Option<String> {
    for pred in TYPE_PREDICATES {
        if type_str.contains(pred) {
            return Some((*pred).to_string());
        }
    }
    None
}

/// Search for a name in contract files under the `contracts/` directory.
///
/// Uses `find_arrows_root` to locate the arrows root from `file_path`, then
/// looks for the sibling `contracts/` directory. First searches for
/// `axiom <name>` patterns, then falls back to searching for the bare name
/// as a substring (to find predicate references in `requires:`/`guarantee:`
/// clauses).
///
/// Returns an LSP `Location` JSON object pointing to the first match, or
/// `None` if no contract file contains the name.
fn search_in_contracts(file_path: Option<&str>, name: &str) -> Option<Value> {
    let path = file_path?;
    let arrows_root = find_arrows_root(path)?;
    let contracts_dir = contracts_dir_from_arrows_root(&arrows_root)?;

    // First, try `axiom <name>` — this matches axiom declarations.
    let axiom_needle = format!("axiom {}", name);
    if let Some(loc) = search_text_in_contracts(Path::new(&contracts_dir), &axiom_needle, name) {
        return Some(loc);
    }

    // Fall back to the bare name — this matches predicate references in
    // `requires:`/`guarantee:` clauses (e.g. `Sorted(output)`).
    search_text_in_contracts(Path::new(&contracts_dir), name, name)
}

/// Recursively search `.hnl` files under `dir` for `needle`, returning an
/// LSP `Location` JSON object pointing to the first match.
///
/// The `name` parameter controls the width of the range (the end character
/// is `start_character + name.len()`).
fn search_text_in_contracts(dir: &Path, needle: &str, name: &str) -> Option<Value> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(loc) = search_text_in_contracts(&path, needle, name) {
                return Some(loc);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("hnl") {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Some(byte_offset) = contents.find(needle) {
                    let uri = path_to_uri(&path);
                    let (line, character) = offset_to_position(&contents, byte_offset);
                    let end_char = character + name.len() as u32;
                    return Some(json!({
                        "uri": uri,
                        "range": {
                            "start": { "line": line, "character": character },
                            "end": { "line": line, "character": end_char },
                        }
                    }));
                }
            }
        }
    }
    None
}

/// Compute the `contracts/` directory path that is a child of the kit root.
///
/// Given `.../handl-packages/`, returns
/// `.../handl-packages/contracts/`.
fn contracts_dir_from_arrows_root(arrows_root: &str) -> Option<String> {
    let root = Path::new(arrows_root);
    let contracts = root.join("contracts");
    if contracts.is_dir() {
        Some(contracts.to_string_lossy().into_owned())
    } else {
        None
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

// ---------------------------------------------------------------------------
// Helpers: rec state field parsing and product type component extraction.
//
// These mirror the private helpers in `refinement.rs` and `inlay_hints.rs`.
// They are duplicated here to avoid modifying `refinement.rs` (which would
// widen its public API) and to keep this module self-contained.
// ---------------------------------------------------------------------------

/// Parse the rec state declaration from an arrow file to extract field
/// names in order.
///
/// For example, given:
/// ```text
/// rec {
///   remaining: input,
///   acc:       []
/// } (
/// ```
///
/// Returns `Some(vec!["remaining", "acc"])`.
fn parse_rec_state_fields(source: &str) -> Option<Vec<String>> {
    let rec_start = source.find("rec")?;
    let after_rec = &source[rec_start..];

    let brace_start = after_rec.find('{')?;
    let after_brace = &after_rec[brace_start + 1..];

    let brace_end = after_brace.find('}')?;
    let state_block = &after_brace[..brace_end];

    let mut fields = Vec::new();
    for line in state_block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(colon_idx) = trimmed.find(':') {
            let name = trimmed[..colon_idx].trim();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                fields.push(name.to_string());
            }
        }
    }

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

/// Extract the nth component from a product type `RefTy`.
///
/// For `Prod(a, b)`, position 0 returns `a`, position 1 returns `b`.
/// For nested products like `Prod(Prod(a, b), c)`, position 0 returns `a`,
/// position 1 returns `b`, position 2 returns `c`.
/// Returns `None` if the position is out of bounds or the type is not a
/// product.
fn extract_product_component(ty: &RefTy, pos: usize) -> Option<RefTy> {
    extract_component_helper(ty, pos).map(|(component, _)| component)
}

/// Helper: returns (component_at_pos, remaining_pos) or None.
fn extract_component_helper(ty: &RefTy, pos: usize) -> Option<(RefTy, usize)> {
    match ty {
        RefTy::Prod(fst, snd) => {
            if pos == 0 {
                if let Some((inner, _)) = extract_component_helper(fst, 0) {
                    Some((inner, 0))
                } else {
                    Some((fst.as_ref().clone(), 0))
                }
            } else {
                if let Some((component, remaining)) = extract_component_helper(fst, pos) {
                    return Some((component, remaining));
                }
                extract_component_helper(snd, pos - 1)
            }
        }
        _ => {
            if pos == 0 {
                Some((ty.clone(), 0))
            } else {
                None
            }
        }
    }
}

// Suppress unused-import warnings for tokens used only in type annotations.
#[allow(dead_code)]
fn _unused_token_imports(_: &[SpannedToken]) {}
