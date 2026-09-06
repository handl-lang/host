//! `textDocument/implementation` — jump from an axiom/trait to its arrow
//! implementations.
//!
//! When the cursor rests on an axiom name in a contract file (under
//! `contracts/`), this module searches every arrow file under the sibling
//! `arrows/` root and uses the refinement engine to infer each arrow's
//! guarantee. Any arrow whose inferred guarantee *references* the axiom name
//! (e.g. the guarantee `Sorted(output)` references the axiom `Sorted`) is
//! returned as an LSP `Location` pointing at that arrow file.
//!
//! This gives "go to implementation" semantics: from `axiom Sorted : …` you
//! can jump to the arrow files (`sort.hnl`, `insert.hnl`, …) whose inferred
//! contract satisfies the `Sorted` predicate.
//!
//! The matching is deliberately conservative: an arrow "implements" a
//! contract when the pretty-printed inferred guarantee contains the axiom
//! name applied to an argument (`Name(`). Predicates whose pretty form is
//! operator-based (e.g. `LengthEq` renders as `|a| = |b|`) are not matched
//! by name; callers interested in those should extend the matcher.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, is_comment_kind, is_identifier_kind};

use crate::kit_resolver::kit_for_file;
use crate::refinement::{RefinementEngine, find_arrows_root};

/// Return LSP `Location` objects for the arrow files that implement the
/// axiom/trait at the cursor position.
///
/// `source` is the text of the current document, `offset` is the byte offset
/// of the cursor within it, and `file_path` is the filesystem path (or
/// `file://` URI) of the current document — used to determine whether the
/// cursor is in a contract file and to locate the sibling `arrows/` root.
///
/// Returns an empty vector when:
/// - the cursor is not on an identifier,
/// - the current file is not a contract file,
/// - the identifier is not an axiom name (not preceded by `axiom`),
/// - no arrows root can be located, or
/// - no arrow file's inferred guarantee references the axiom name.
pub fn implementation(source: &str, offset: usize, file_path: Option<&str>) -> Vec<Value> {
    // Lex the source once and reuse the token stream for both the identifier
    // lookup and the axiom-name check. This avoids constructing two separate
    // `Kit` runtimes (which are expensive — each assembles a full engine
    // registry with lex/parse/eval/typecheck/compile engines and many module
    // registries) for a single `implementation` request.
    let kit = match kit_for_file(file_path) {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    // a. Find the identifier at the cursor position.
    let name = match identifier_at_cursor(&tokens, offset) {
        Some(n) => n,
        None => return Vec::new(),
    };

    // b. Only meaningful in a contract file, on an axiom name.
    let path = match file_path {
        Some(p) => p,
        None => return Vec::new(),
    };
    let fs_path = uri_to_path(path);
    if !is_contract_file(&fs_path) {
        return Vec::new();
    }
    if !is_axiom_name_at_cursor(&tokens, offset) {
        return Vec::new();
    }

    // c. Locate the arrows root from the contract file path.
    let arrows_root = match arrows_root_from_contracts_path(&fs_path) {
        Some(r) => r,
        None => return Vec::new(),
    };

    // d. For each arrow file, infer its guarantee and check whether it
    //    references the axiom name.
    let engine = RefinementEngine::new();
    let mut locations = Vec::new();
    for arrow_path in collect_arrow_files(&arrows_root) {
        let arrow_source = match fs::read_to_string(&arrow_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let path_str = arrow_path.to_string_lossy();
        if let Some(Ok(result)) = engine.infer_document(&arrow_source, Some(&path_str)) {
            let guarantee = result.guarantee.simplify().pretty();
            if guarantee_references_axiom(&guarantee, &name) {
                locations.push(location_for_file(&arrow_path));
            }
        }
    }

    locations
}

/// Find the identifier token covering `offset`, returning its text.
///
/// Returns `None` when the cursor is not within an identifier token (e.g. on
/// whitespace, a keyword, or an operator).
fn identifier_at_cursor(tokens: &[SpannedToken], offset: usize) -> Option<String> {
    let token = tokens
        .iter()
        .find(|t| t.span.start <= offset && offset < t.span.end)?;
    if !is_identifier_kind(token.kind.as_str()) {
        return None;
    }
    Some(token.text.clone())
}

/// Determine whether the identifier at `offset` is an axiom name — i.e. the
/// identifier (possibly the first or later segment of a dotted name like
/// `list.cons`) is preceded, skipping comments, by the `axiom` keyword.
///
/// `axiom` may be tokenized as either a keyword or a plain identifier
/// depending on the data-driven lexer configuration, so the check is on the
/// token *text* rather than its kind.
fn is_axiom_name_at_cursor(tokens: &[SpannedToken], offset: usize) -> bool {
    let idx = match tokens
        .iter()
        .position(|t| t.span.start <= offset && offset < t.span.end)
    {
        Some(i) => i,
        None => return false,
    };
    if !is_identifier_kind(tokens[idx].kind.as_str()) {
        return false;
    }

    // Walk backwards through a dotted name (ident (. ident)*) and verify the
    // chain originates at the `axiom` keyword/identifier.
    let mut j = idx;
    loop {
        match prev_non_comment_index(&tokens, j) {
            // Directly preceded by `axiom` — this is the first segment.
            Some(pi) if tokens[pi].text == "axiom" => return true,
            // Preceded by `.` — a later segment of a dotted axiom name; the
            // token before the dot must be an identifier to continue.
            Some(pi) if tokens[pi].kind == "token.dot" => {
                match prev_non_comment_index(&tokens, pi) {
                    Some(qi) if is_identifier_kind(tokens[qi].kind.as_str()) => {
                        j = qi;
                        continue;
                    }
                    _ => return false,
                }
            }
            _ => return false,
        }
    }
}

/// Return the index of the previous non-comment token before `from`, or
/// `None` when there is none.
fn prev_non_comment_index(tokens: &[SpannedToken], from: usize) -> Option<usize> {
    let mut j = from;
    while j > 0 {
        j -= 1;
        if !is_comment_kind(tokens[j].kind.as_str()) {
            return Some(j);
        }
    }
    None
}

/// Whether `path` is inside a `contracts/` directory.
fn is_contract_file(path: &str) -> bool {
    path.contains("/contracts/") || path.ends_with("/contracts")
}

/// Convert a `file://` URI to a filesystem path, leaving plain paths untouched.
fn uri_to_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("file://") {
        stripped.to_string()
    } else {
        path.to_string()
    }
}

/// Compute the handl-packages root directory that is the parent of the
/// `contracts/` directory containing `contract_path`.
///
/// Given `…/handl-packages/contracts/list.hnl`, returns
/// `…/handl-packages/` if that directory exists. Falls back to
/// `find_arrows_root` (which scans for `/handl-packages/` in the path)
/// when no `/contracts/` segment is present.
fn arrows_root_from_contracts_path(contract_path: &str) -> Option<String> {
    if let Some(idx) = contract_path.find("/contracts/") {
        let root = &contract_path[..idx];
        let root = format!("{}/", root);
        if Path::new(&root).is_dir() {
            return Some(root);
        }
    }
    // Fallback: maybe the path itself is under handl-packages/ (defensive).
    find_arrows_root(contract_path)
}

/// Recursively collect every `.hnl` file under `root`, sorted for stable
/// output ordering. The `contracts/` subdirectory is skipped.
fn collect_arrow_files(root: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_dir(Path::new(root), &mut out);
    out.sort();
    out
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) {
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
            collect_dir(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("hnl") {
            out.push(path);
        }
    }
}

/// Check whether the pretty-printed guarantee references the axiom name.
///
/// Matches `Name(` so that the axiom `Sorted` is matched by the guarantee
/// `Sorted(output)` but a bare substring like `Sort` is not. This suits
/// predicate-style axioms (`Sorted`, `AllSatisfy`, `HeadOf`, `IsPerm`,
/// `IsSuccess`, `IsError`); operator-rendered predicates such as `LengthEq`
/// (printed `|a| = |b|`) are not matched by name.
fn guarantee_references_axiom(guarantee: &str, axiom_name: &str) -> bool {
    let needle = format!("{}(", axiom_name);
    guarantee.contains(&needle)
}

/// Build an LSP `Location` JSON object pointing at the start of `path`.
fn location_for_file(path: &Path) -> Value {
    let uri = path_to_uri(path);
    json!({
        "uri": uri,
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        }
    })
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
