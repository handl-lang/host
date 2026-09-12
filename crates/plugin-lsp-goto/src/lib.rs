//! `textDocument/definition` plugin for the handl LSP server.
//!
//! Resolves in-file binding definitions and falls back to cross-file
//! arrow/axiom definitions under the `handl-packages/` root.

use std::fs;
use std::path::Path;

use handl_host_tooling::lsp::{GotoDefPlugin, GotoDefRequest, GotoDefResponse, Location};
use handl_kit_canonical::{Kit, SpannedToken, TokenClasses};
use handl_language::Span;
use handl_platform::PlatformError;

/// A goto-definition plugin backed by the canonical kit's lexer.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicGotoDef;

impl BasicGotoDef {
    pub fn new() -> Self {
        Self
    }
}

impl GotoDefPlugin for BasicGotoDef {
    fn run(&self, request: &GotoDefRequest) -> Result<GotoDefResponse, PlatformError> {
        Ok(GotoDefResponse(resolve(request)))
    }
}

fn resolve(request: &GotoDefRequest) -> Option<Location> {
    let (name, _span) = identifier_at_offset(&request.source, request.pos)?;

    if let Some(def_span) = find_definition(&request.source, &name) {
        return Some(Location::new(def_span.start, def_span.end));
    }

    if let Some(uri) = find_cross_file_definition(&name, &request.path) {
        let mut loc = Location::new(0, 0);
        loc.uri = Some(uri);
        return Some(loc);
    }

    None
}

/// Return the identifier token that contains `offset`, if any.
fn identifier_at_offset(source: &str, offset: usize) -> Option<(String, Span)> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    for t in &tokens {
        if classes.is_identifier(&t.kind) {
            if t.span.start <= offset && offset < t.span.end {
                return Some((t.text.clone(), t.span));
            }
        }
    }
    None
}

/// Find an in-file definition of `name` where an identifier is followed by
/// the `::` label (a binding definition).
fn find_definition(source: &str, name: &str) -> Option<Span> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let mut i = 0;
    while i < tokens.len() {
        if classes.is_identifier(&tokens[i].kind) && tokens[i].text == name {
            let mut j = i + 1;
            while j < tokens.len() && classes.is_comment(&tokens[j].kind) {
                j += 1;
            }
            if j < tokens.len() && classes.is_label(&tokens[j].kind, "::") {
                return Some(tokens[i].span);
            }
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Cross-file arrow / axiom resolution
// ---------------------------------------------------------------------------

/// Find the arrows root directory from a file path.
///
/// Looks for `/handl-packages/` in the path and returns everything up to and
/// including it. Returns `None` if the path doesn't contain `/handl-packages/`.
fn find_arrows_root(path: &str) -> Option<String> {
    let marker = "/handl-packages/";
    let idx = path.find(marker)?;
    Some(path[..idx + marker.len()].to_string())
}

/// Given an identifier name and the current file path, search for a `.hnl`
/// file named `name.hnl` in the arrows directory tree.
fn find_cross_file_definition(name: &str, file_path: &str) -> Option<String> {
    let arrows_root = find_arrows_root(file_path)?;

    if let Some(uri) = find_arrow_file_by_name(name, &arrows_root) {
        return Some(uri);
    }

    if name.contains('.') {
        if let Some(uri) = find_axiom_definition(name, &arrows_root) {
            return Some(uri);
        }
    }

    None
}

fn find_arrow_file_by_name(name: &str, arrows_root: &str) -> Option<String> {
    let target = format!("{}.hnl", name);
    let root = Path::new(arrows_root);
    if !root.is_dir() {
        return None;
    }
    search_for_file(root, &target)
}

fn search_for_file(dir: &Path, file_name: &str) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
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

fn find_axiom_definition(name: &str, arrows_root: &str) -> Option<String> {
    let contracts_dir = contracts_dir_from_arrows_root(arrows_root)?;
    let needle = format!("axiom {}", name);
    search_for_text_in_hnl(Path::new(&contracts_dir), &needle)
}

fn contracts_dir_from_arrows_root(arrows_root: &str) -> Option<String> {
    let root = Path::new(arrows_root);
    let contracts = root.join("contracts");
    if contracts.is_dir() {
        Some(contracts.to_string_lossy().into_owned())
    } else {
        None
    }
}

fn search_for_text_in_hnl(dir: &Path, needle: &str) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
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
