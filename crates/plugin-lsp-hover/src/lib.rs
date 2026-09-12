//! `textDocument/hover` plugin for the handl LSP server.
//!
//! Provides token-level markdown hover text for canonical `.hnl` source.
//! Cross-file arrow refinement hovers stay in the host (`handl-host-lsp`).

use std::fs;
use std::path::{Path, PathBuf};

use handl_host_tooling::lsp::{HoverPlugin, HoverRequest, HoverResponse};
use handl_kit_canonical::{Kit, SpannedToken, TokenClasses};
use handl_language::Span;
use handl_platform::PlatformError;

/// A hover plugin backed by the canonical kit's lexer.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicHover;

impl BasicHover {
    pub fn new() -> Self {
        Self
    }
}

impl HoverPlugin for BasicHover {
    fn run(&self, request: &HoverRequest) -> Result<HoverResponse, PlatformError> {
        Ok(HoverResponse(token_hover(&request.source, request.pos, &request.path)))
    }
}

fn token_hover(source: &str, pos: usize, path: &str) -> Option<String> {
    if let Some(md) = import_hover(source, pos, path) {
        return Some(md);
    }
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.span.start <= pos && pos < t.span.end)?;
    classify_token(&classes, &tokens, index, token)
        .or_else(|| extract_doc_comments(source))
}

/// Extract leading `---` doc comments as markdown text.
fn extract_doc_comments(source: &str) -> Option<String> {
    let lines = leading_doc_lines(source);
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn leading_doc_lines(source: &str) -> Vec<String> {
    let kit = match Kit::new() {
        Ok(k) => k,
        Err(_) => return Vec::new(),
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let classes = TokenClasses::new(kit.language());
    let mut lines: Vec<String> = Vec::new();
    for t in &tokens {
        if classes.is_doc_comment(t.kind.as_str()) {
            let text = t.text.strip_prefix("---").unwrap_or(&t.text);
            let text = text.strip_prefix(' ').unwrap_or(text);
            lines.push(text.to_string());
        } else if classes.is_comment(t.kind.as_str()) {
            continue;
        } else {
            break;
        }
    }
    lines
}

fn classify_token(
    classes: &TokenClasses,
    tokens: &[SpannedToken],
    index: usize,
    token: &SpannedToken,
) -> Option<String> {
    let kind = token.kind.as_str();

    if classes.is_bracket(kind) {
        Some(format!("**{}**", token.text))
    } else if classes.is_number(kind) {
        Some(format!("**number** `{}`", token.text))
    } else if classes.is_string(kind) {
        let inner = &token.text[1..token.text.len().saturating_sub(1)];
        Some(format!("**string** `{}`", escape_string(inner)))
    } else if classes.is_bool(kind) {
        Some(format!("**boolean** `{}`", token.text))
    } else if classes.is_null(kind) {
        Some("**null** literal".to_string())
    } else if classes.is_keyword(kind) {
        Some(keyword_hover(classes, kind, &token.text))
    } else if classes.is_identifier(kind) {
        Some(identifier_hover(classes, tokens, index, token))
    } else if classes.is_operator(kind) {
        Some(operator_hover(&token.text))
    } else if classes.is_delimiter(kind) {
        Some(format!("**punctuation** `{}`", token.text))
    } else if classes.is_doc_comment(kind) {
        Some("**doc comment**".to_string())
    } else if classes.is_comment(kind) {
        Some("**comment**".to_string())
    } else if classes.is_template(kind) {
        Some("**template string**".to_string())
    } else {
        None
    }
}

fn keyword_hover(classes: &TokenClasses, kind: &str, text: &str) -> String {
    classes
        .doc_of(kind)
        .map(|d| d.to_string())
        .unwrap_or_else(|| format!("`keyword` — `{}`", text))
}

fn identifier_hover(
    classes: &TokenClasses,
    tokens: &[SpannedToken],
    index: usize,
    token: &SpannedToken,
) -> String {
    if let Some(md) = arrow_combinator_hover(classes, tokens, index, token) {
        return md;
    }
    let mut md = format!("**{}**", token.text);
    if index > 0 && classes.is_label(tokens[index - 1].kind.as_str(), ".") {
        md.push_str("\n\nProperty / field access");
    } else if index + 1 < tokens.len() && classes.is_label(tokens[index + 1].kind.as_str(), "(") {
        md.push_str("\n\nFunction call");
    }
    md
}

fn arrow_combinator_hover(
    classes: &TokenClasses,
    tokens: &[SpannedToken],
    index: usize,
    token: &SpannedToken,
) -> Option<String> {
    let is_call = tokens
        .get(index + 1)
        .map(|t| classes.is_label(t.kind.as_str(), "("))
        .unwrap_or(false);
    if !is_call && token.text != "id" {
        return None;
    }
    match token.text.as_str() {
        "arr" => Some("**`arr`** — lift a pure function into an arrow".to_string()),
        "comp" | "seq" => {
            Some("**`comp` / `seq`** — sequential arrow composition".to_string())
        }
        "first" => Some("**`first`** — run a sub-arrow on the first component".to_string()),
        "choice" => Some("**`choice`** — route on `Either`".to_string()),
        "app" => Some("**`app`** — arrow application".to_string()),
        "loop" => Some("**`loop`** — feedback loop".to_string()),
        "id" => Some("**`id`** — identity arrow".to_string()),
        _ => None,
    }
}

fn operator_hover(text: &str) -> String {
    let name = match text {
        "=" | "==" => "equality",
        "!=" => "inequality",
        "<" => "less-than",
        ">" => "greater-than",
        "<=" => "less-than-or-equal",
        ">=" => "greater-than-or-equal",
        "+" => "addition",
        "-" => "subtraction",
        "*" => "multiplication",
        "/" => "division",
        "%" => "modulo",
        "**" => "power",
        "&&" => "logical-and",
        "||" => "logical-or",
        "~>" => "arrow",
        ">>>" => "sequential composition",
        "***" => "parallel product",
        "&&&" => "fan-out",
        "|||" => "choice",
        "??" => "nullish coalescing",
        "?." => "optional chaining",
        ":" => "type annotation",
        "." => "field access",
        "..." => "spread / rest",
        _ => "operator",
    };
    format!("**{}** `{}`", name, text)
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------
// Import-driven hover
// ---------------------------------------------------------------------------

fn import_hover(source: &str, pos: usize, path: &str) -> Option<String> {
    let current_dir = base_dir(path)?;
    let (name, _span) = identifier_at_offset(source, pos)?;

    if let Some((target_path, full)) = find_import_for_name(source, &name, &current_dir) {
        let target = fs::read_to_string(&target_path).ok()?;
        if let Some(md) = export_hover(&target, &full) {
            return Some(md);
        }
    }

    if let Some((mod_name, member)) = dotted_pair_at(source, pos) {
        if let Some(target_path) = find_module_import(source, &mod_name, &current_dir) {
            let target = fs::read_to_string(&target_path).ok()?;
            if let Some(md) = export_hover(&target, &member) {
                return Some(md);
            }
        }
    }

    None
}

fn export_hover(source: &str, needle: &str) -> Option<String> {
    let (doc, signature) = export_doc_and_signature(source, needle)?;
    Some(format!("{}\n\n```\n{}\n```", doc, signature))
}

fn export_doc_and_signature(source: &str, needle: &str) -> Option<(String, String)> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let info = find_export(&tokens, &classes, needle)?;
    let doc = export_doc(&tokens, &classes, &info, source)?;
    let signature = export_signature(&tokens, &classes, &info);
    Some((doc, signature))
}

#[derive(Debug, Clone, Copy)]
enum ExportKind {
    Axiom,
    Law,
    Arrow,
    Variable,
}

struct ExportInfo {
    full: String,
    #[allow(dead_code)]
    span: Span,
    decl_index: usize,
    kind: ExportKind,
}

fn find_export(tokens: &[SpannedToken], classes: &TokenClasses, needle: &str) -> Option<ExportInfo> {
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
        if classes.is_keyword(&tokens[i].kind) {
            match tokens[i].text.as_str() {
                "axiom" | "law" | "export" => {
                    if let Some(name) = export_name_at(&tokens, i + 1, &classes) {
                        if &name.full == needle || last_part(&name.full) == needle {
                            let kind = match tokens[i].text.as_str() {
                                "axiom" => ExportKind::Axiom,
                                "law" => ExportKind::Law,
                                _ => ExportKind::Arrow,
                            };
                            return Some(ExportInfo {
                                full: name.full,
                                span: name.span,
                                decl_index: i,
                                kind,
                            });
                        }
                    }
                }
                _ => {}
            }
        } else if classes.is_identifier(&tokens[i].kind) {
            if let Some(next) = tokens.get(i + 1) {
                if classes.is_label(&next.kind, "=") || classes.is_label(&next.kind, "::") {
                    if &tokens[i].text == needle {
                        return Some(ExportInfo {
                            full: tokens[i].text.clone(),
                            span: tokens[i].span,
                            decl_index: i,
                            kind: ExportKind::Variable,
                        });
                    }
                }
            }
        }
        i += 1;
    }
    None
}

struct Name {
    full: String,
    span: Span,
}

fn export_name_at(
    tokens: &[SpannedToken],
    start: usize,
    classes: &TokenClasses,
) -> Option<Name> {
    if let Some((full, span)) = parse_dotted_name(tokens, start, classes) {
        return Some(Name { full, span });
    }
    let t = tokens.get(start)?;
    if classes.is_identifier(&t.kind) {
        Some(Name {
            full: t.text.clone(),
            span: t.span,
        })
    } else {
        None
    }
}

fn full_exported_name(source: &str, needle: &str) -> Option<String> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    find_export(&tokens, &classes, needle).map(|e| e.full)
}

fn export_doc(
    tokens: &[SpannedToken],
    classes: &TokenClasses,
    info: &ExportInfo,
    source: &str,
) -> Option<String> {
    if matches!(info.kind, ExportKind::Axiom | ExportKind::Law) {
        if let Some(doc) = preceding_comment_lines(tokens, classes, info.decl_index, &info.full) {
            return Some(doc);
        }
    }
    fallback_doc(source, &info.full)
}

fn preceding_comment_lines(
    tokens: &[SpannedToken],
    classes: &TokenClasses,
    start: usize,
    name: &str,
) -> Option<String> {
    let mut lines = Vec::new();
    let mut i = start as isize - 1;
    while i >= 0 {
        let t = &tokens[i as usize];
        if classes.is_comment(t.kind.as_str()) && !classes.is_doc_comment(t.kind.as_str()) {
            let text = t.text.strip_prefix("--").unwrap_or(&t.text);
            let text = text.strip_prefix(' ').unwrap_or(text);
            lines.push(text.to_string());
            i -= 1;
        } else {
            break;
        }
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    let doc = lines.join("\n");
    let needle = last_part(name);
    if doc.contains(name) || doc.contains(needle) {
        Some(doc)
    } else {
        None
    }
}

fn fallback_doc(source: &str, name: &str) -> Option<String> {
    let lines = leading_doc_lines(source);
    if lines.is_empty() {
        return None;
    }
    let needle = last_part(name);
    if lines.iter().any(|l| l.contains(name) || l.contains(needle)) {
        Some(lines.join("\n"))
    } else {
        Some(lines.iter().take(5).cloned().collect::<Vec<_>>().join("\n"))
    }
}

fn export_signature(tokens: &[SpannedToken], _classes: &TokenClasses, info: &ExportInfo) -> String {
    if matches!(info.kind, ExportKind::Axiom | ExportKind::Law | ExportKind::Arrow) {
        return "Type ~> Type".to_string();
    }
    for t in tokens.iter().skip(info.decl_index + 1) {
        if t.text == "axiom" || t.text == "law" || t.text == "export" {
            break;
        }
        if t.text == "~>" || t.text == "->" {
            return "Type ~> Type".to_string();
        }
    }
    "Type".to_string()
}

// ---------------------------------------------------------------------------
// Import resolution (adapted from plugin-lsp-goto)
// ---------------------------------------------------------------------------

/// Return the identifier token that contains `offset`, if any.
fn identifier_at_offset(source: &str, offset: usize) -> Option<(String, Span)> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    for t in &tokens {
        if classes.is_identifier(&t.kind) && t.span.start <= offset && offset < t.span.end {
            return Some((t.text.clone(), t.span));
        }
    }
    None
}

/// Resolve a canonical import specifier to a concrete `.hnl` file.
fn resolve_import_target(spec: &str, current_dir: &Path) -> Option<PathBuf> {
    let p = current_dir.join(spec);
    if p.is_file() {
        return Some(p);
    }
    let with_ext = p.with_extension("hnl");
    if with_ext.is_file() {
        return Some(with_ext);
    }
    if p.is_dir() {
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            let child = p.join(format!("{name}.hnl"));
            if child.is_file() {
                return Some(child);
            }
        }
        let child = p.join("mod.hnl");
        if child.is_file() {
            return Some(child);
        }
    }
    None
}

/// Parse the current source's import statements and resolve the first
/// canonical import that exports `name`.
fn find_import_for_name(source: &str, name: &str, current_dir: &Path) -> Option<(PathBuf, String)> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let mut i = 0;
    while i < tokens.len() {
        if classes.is_keyword(&tokens[i].kind) && tokens[i].text == "from" {
            let spec_tok = tokens.get(i + 1)?;
            if !classes.is_string(&spec_tok.kind) {
                i += 1;
                continue;
            }
            let import_tok = tokens.get(i + 2)?;
            if !classes.is_keyword(&import_tok.kind) || import_tok.text != "import" {
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

        if classes.is_keyword(&tokens[i].kind) && tokens[i].text == "import" {
            let name_tok = tokens.get(i + 1)?;
            if !classes.is_identifier(&name_tok.kind) {
                i += 1;
                continue;
            }
            let from_tok = tokens.get(i + 2)?;
            if !classes.is_keyword(&from_tok.kind) || from_tok.text != "from" {
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

/// Find a whole-module import that binds the module under `mod_name`.
fn find_module_import(source: &str, mod_name: &str, current_dir: &Path) -> Option<PathBuf> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    let mut i = 0;
    while i < tokens.len() {
        if classes.is_keyword(&tokens[i].kind) && tokens[i].text == "from" {
            let spec_tok = tokens.get(i + 1)?;
            if !classes.is_string(&spec_tok.kind) {
                i += 1;
                continue;
            }
            let import_tok = tokens.get(i + 2)?;
            if !classes.is_keyword(&import_tok.kind) || import_tok.text != "import" {
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

        if classes.is_keyword(&tokens[i].kind) && tokens[i].text == "import" {
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

fn base_dir(path: &str) -> Option<PathBuf> {
    Path::new(path).parent().and_then(|p| {
        if p.as_os_str().is_empty() {
            std::env::current_dir().ok()
        } else {
            Some(p.to_path_buf())
        }
    })
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
fn parse_dotted_name(
    tokens: &[SpannedToken],
    start: usize,
    classes: &TokenClasses,
) -> Option<(String, Span)> {
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
