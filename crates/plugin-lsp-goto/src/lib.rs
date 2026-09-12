//! `textDocument/definition` plugin for the handl LSP server.
//!
//! Resolves in-file binding definitions and falls back to cross-file
//! arrow/axiom definitions under the `handl-packages/` root.
//! Import-based resolution is handled before either fallback.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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
    let current_dir = base_dir(&request.path)?;
    if let Some(loc) = import_spec_goto(&request.source, request.pos, &current_dir) {
        return Some(loc);
    }
    let (name, _span) = identifier_at_offset(&request.source, request.pos)?;

    // 1. Canonical named import resolution.
    if let Some((path, full)) = find_import_for_name(&request.source, &name, &current_dir) {
        let target = fs::read_to_string(&path).ok()?;
        let span = span_of_exported_name(&target, &full)?;
        let mut loc = Location::new(span.start, span.end);
        loc.uri = Some(path_to_uri(&path));
        return Some(loc);
    }

    // 2. Whole-module import resolution for `mod.name` dotted access.
    if let Some((mod_name, member)) = dotted_pair_at(&request.source, request.pos) {
        if let Some(path) = find_module_import(&request.source, &mod_name, &current_dir) {
            let target = fs::read_to_string(&path).ok()?;
            let span = span_of_exported_name(&target, &member)?;
            let mut loc = Location::new(span.start, span.end);
            loc.uri = Some(path_to_uri(&path));
            return Some(loc);
        }
    }

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

/// Goto the module file when the cursor sits on an import specifier string.
fn import_spec_goto(source: &str, pos: usize, current_dir: &Path) -> Option<Location> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let idx = tokens
        .iter()
        .position(|t| t.span.start <= pos && pos < t.span.end)?;
    let tok = &tokens[idx];
    if !classes.is_string(&tok.kind) {
        return None;
    }
    let prev = tokens.get(idx.checked_sub(1)?)?;
    if prev.text != "import" && prev.text != "from" {
        return None;
    }
    let spec = strip_quotes(&tok.text);
    let path = resolve_import_target(spec, current_dir)?;
    let mut loc = Location::new(0, 0);
    loc.uri = Some(path_to_uri(&path));
    Some(loc)
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
// Import resolution
// ---------------------------------------------------------------------------

// TODO: `resolve_import_target` and `collect_exports` are duplicated from
// `plugin-lsp-completion`. Consider moving them to `handl-host-tooling` or
// a small `plugin-lsp-commons` crate if more LSP plugins need them.

/// Resolve a canonical import specifier to a concrete `.hnl` file.
///
/// Tries the raw path, the same path with the `.hnl` extension, then the
/// canonical module layouts `spec/spec.hnl` and `spec/mod.hnl`.
fn resolve_import_target(spec: &str, current_dir: &Path) -> Option<PathBuf> {
    let p = current_dir.join(spec);
    let p = p.canonicalize().ok().unwrap_or(p);
    if p.is_file() {
        return Some(p);
    }
    if p.extension().is_none() {
        let with_ext = p.with_extension("hnl");
        let with_ext = with_ext.canonicalize().ok().unwrap_or(with_ext);
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }
    let dir = if p.is_dir() { p.clone() } else { p.with_extension("") };
    let dir = dir.canonicalize().ok().unwrap_or(dir);
    if dir.is_dir() {
        if let Some(name) = dir.file_stem().and_then(|s| s.to_str()) {
            let child = dir.join(format!("{name}.hnl"));
            let child = child.canonicalize().ok().unwrap_or(child);
            if child.is_file() {
                return Some(child);
            }
        }
        let child = dir.join("mod.hnl");
        let child = child.canonicalize().ok().unwrap_or(child);
        if child.is_file() {
            return Some(child);
        }
    }
    None
}

/// Scan a target `.hnl` source and return all exported top-level names.
///
/// Dotted names such as `axiom time.now_ms` are returned as both the full
/// dotted form and the last segment.
#[allow(dead_code)]
fn collect_exports(source: &str) -> Vec<String> {
    let kit = match Kit::new() {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let classes = TokenClasses::new(kit.language());
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut depth = 0;
    let mut i = 0;
    while i < tokens.len() {
        let kind = tokens[i].kind.as_str();
        if classes.is_open_bracket(kind) {
            depth += 1;
        } else if classes.is_close_bracket(kind) {
            depth -= 1;
        }
        if depth != 0 {
            i += 1;
            continue;
        }
        if classes.is_keyword(&tokens[i].kind)
            && (tokens[i].text == "axiom" || tokens[i].text == "law")
        {
            if let Some((full, _)) = parse_dotted_name(&tokens, i + 1, &classes) {
                if seen.insert(full.clone()) {
                    names.push(full.clone());
                }
                if let Some(last) = full.rsplit_once('.').map(|(_, l)| l) {
                    if seen.insert(last.to_string()) {
                        names.push(last.to_string());
                    }
                }
            }
        } else if classes.is_identifier(&tokens[i].kind) {
            if let Some(next) = tokens.get(i + 1) {
                if classes.is_label(&next.kind, "=") || classes.is_label(&next.kind, "::") {
                    if seen.insert(tokens[i].text.clone()) {
                        names.push(tokens[i].text.clone());
                    }
                }
            }
        }
        i += 1;
    }
    names
}

/// Parse the current source's import statements and resolve the first
/// canonical import that exports `name`.
///
/// Supported forms:
///   - `from "<spec>" import <name> [as <alias>]`
///   - `import <name> from "<spec>"` (arrow style)
///
/// If a dotted export such as `time.now_ms` matches the last segment `now_ms`,
/// the file path and the full dotted token text are returned.
fn find_import_for_name(source: &str, name: &str, current_dir: &Path) -> Option<(PathBuf, String)> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].text == "from" {
            let spec_tok = tokens.get(i + 1)?;
            if !classes.is_string(&spec_tok.kind) {
                i += 1;
                continue;
            }
            let import_tok = tokens.get(i + 2)?;
            if import_tok.text != "import" {
                i += 1;
                continue;
            }
            let name_tok = tokens.get(i + 3)?;
            if !classes.is_identifier(&name_tok.kind) {
                i += 1;
                continue;
            }
            let imported = name_tok.text.clone();
            let alias = tokens
                .get(i + 4)
                .filter(|t| classes.is_keyword(&t.kind) && t.text == "as")
                .and_then(|_| tokens.get(i + 5))
                .filter(|t| classes.is_identifier(&t.kind));
            if &imported != name && alias.map(|t| t.text != name).unwrap_or(true) {
                i += 1;
                continue;
            }
            let spec = strip_quotes(&spec_tok.text);
            let path = resolve_import_target(spec, current_dir)?;
            let target = fs::read_to_string(&path).ok()?;
            let full = full_exported_name(&target, &imported)?;
            return Some((path, full));
        }

        if tokens[i].text == "import" {
            let name_tok = tokens.get(i + 1)?;
            if !classes.is_identifier(&name_tok.kind) {
                i += 1;
                continue;
            }
            let from_tok = tokens.get(i + 2)?;
            if from_tok.text != "from" {
                i += 1;
                continue;
            }
            let spec_tok = tokens.get(i + 3)?;
            if !classes.is_string(&spec_tok.kind) {
                i += 1;
                continue;
            }
            let imported = name_tok.text.clone();
            if &imported != name {
                i += 1;
                continue;
            }
            let spec = strip_quotes(&spec_tok.text);
            let path = resolve_import_target(spec, current_dir)?;
            let target = fs::read_to_string(&path).ok()?;
            let full = full_exported_name(&target, &imported)?;
            return Some((path, full));
        }

        i += 1;
    }
    None
}

/// Find a whole-module import (`import "<spec>"` or `from "<spec>" import * as ...`)
/// that binds the module under `mod_name` and resolve it to a file.
fn find_module_import(source: &str, mod_name: &str, current_dir: &Path) -> Option<PathBuf> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].text == "from" {
            let spec_tok = tokens.get(i + 1)?;
            if !classes.is_string(&spec_tok.kind) {
                i += 1;
                continue;
            }
            let import_tok = tokens.get(i + 2)?;
            if import_tok.text != "import" {
                i += 1;
                continue;
            }
            let star_tok = tokens.get(i + 3)?;
            if star_tok.text != "*" {
                i += 1;
                continue;
            }
            let alias = tokens
                .get(i + 4)
                .filter(|t| classes.is_keyword(&t.kind) && t.text == "as")
                .and_then(|_| tokens.get(i + 5))
                .filter(|t| classes.is_identifier(&t.kind));
            let bound = alias
                .map(|t| t.text.clone())
                .unwrap_or_else(|| module_name_from_spec(strip_quotes(&spec_tok.text)));
            if &bound == mod_name {
                let spec = strip_quotes(&spec_tok.text);
                return resolve_import_target(spec, current_dir);
            }
            i += if alias.is_some() { 6 } else { 4 };
            continue;
        }

        if tokens[i].text == "import" {
            let spec_tok = tokens.get(i + 1)?;
            if !classes.is_string(&spec_tok.kind) {
                i += 1;
                continue;
            }
            let alias = tokens
                .get(i + 2)
                .filter(|t| classes.is_keyword(&t.kind) && t.text == "as")
                .and_then(|_| tokens.get(i + 3))
                .filter(|t| classes.is_identifier(&t.kind));
            let bound = alias
                .map(|t| t.text.clone())
                .unwrap_or_else(|| module_name_from_spec(strip_quotes(&spec_tok.text)));
            if &bound == mod_name {
                let spec = strip_quotes(&spec_tok.text);
                return resolve_import_target(spec, current_dir);
            }
            i += if alias.is_some() { 4 } else { 2 };
            continue;
        }

        i += 1;
    }
    None
}

/// If the cursor sits on either side of a `mod.name` dotted access,
/// return `(mod, name)`.
fn dotted_pair_at(source: &str, pos: usize) -> Option<(String, String)> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let idx = tokens
        .iter()
        .position(|t| t.span.start <= pos && pos < t.span.end)?;
    if !classes.is_identifier(&tokens[idx].kind) {
        return None;
    }
    if idx >= 2
        && classes.is_label(&tokens[idx - 1].kind, ".")
        && classes.is_identifier(&tokens[idx - 2].kind)
    {
        return Some((tokens[idx - 2].text.clone(), tokens[idx].text.clone()));
    }
    if idx + 2 < tokens.len()
        && classes.is_label(&tokens[idx + 1].kind, ".")
        && classes.is_identifier(&tokens[idx + 2].kind)
    {
        return Some((tokens[idx].text.clone(), tokens[idx + 2].text.clone()));
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn base_dir(path: &str) -> Option<PathBuf> {
    Path::new(path).parent().and_then(|p| {
        if p.as_os_str().is_empty() {
            std::env::current_dir().ok()
        } else {
            Some(p.to_path_buf())
        }
    })
    .and_then(|p| p.canonicalize().ok().or(Some(p)))
}

fn strip_quotes(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}

fn module_name_from_spec(spec: &str) -> String {
    Path::new(spec)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(spec)
        .to_string()
}

fn last_part(name: &str) -> &str {
    name.rsplit_once('.').map(|(_, l)| l).unwrap_or(name)
}

/// Parse a dotted name starting at `start`, returning the full text and the
/// combined span of the first through last identifier.
fn parse_dotted_name(tokens: &[SpannedToken], start: usize, classes: &TokenClasses) -> Option<(String, Span)> {
    if start >= tokens.len() || !classes.is_identifier(&tokens[start].kind) {
        return None;
    }
    let mut parts = vec![tokens[start].text.clone()];
    let mut end = tokens[start].span.end;
    let mut i = start + 1;
    while i + 1 < tokens.len()
        && classes.is_label(&tokens[i].kind, ".")
        && classes.is_identifier(&tokens[i + 1].kind)
    {
        parts.push(tokens[i + 1].text.clone());
        end = tokens[i + 1].span.end;
        i += 2;
    }
    Some((
        parts.join("."),
        Span {
            start: tokens[start].span.start,
            end,
        },
    ))
}

fn full_exported_name(source: &str, needle: &str) -> Option<String> {
    find_export_info(source, needle).map(|(full, _)| full)
}

fn span_of_exported_name(source: &str, needle: &str) -> Option<Span> {
    find_export_info(source, needle).map(|(_, span)| span)
}

fn find_export_info(source: &str, needle: &str) -> Option<(String, Span)> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let mut depth = 0;
    let mut i = 0;
    while i < tokens.len() {
        let kind = tokens[i].kind.as_str();
        if classes.is_open_bracket(kind) {
            depth += 1;
        } else if classes.is_close_bracket(kind) {
            depth -= 1;
        }
        if depth != 0 {
            i += 1;
            continue;
        }
        if classes.is_keyword(&tokens[i].kind)
            && (tokens[i].text == "axiom" || tokens[i].text == "law")
        {
            if let Some((full, span)) = parse_dotted_name(&tokens, i + 1, &classes) {
                if &full == needle || last_part(&full) == needle {
                    return Some((full, span));
                }
            }
        } else if classes.is_identifier(&tokens[i].kind) {
            if let Some(next) = tokens.get(i + 1) {
                if classes.is_label(&next.kind, "=") || classes.is_label(&next.kind, "::") {
                    if &tokens[i].text == needle {
                        return Some((tokens[i].text.clone(), tokens[i].span));
                    }
                }
            }
        }
        i += 1;
    }
    None
}
