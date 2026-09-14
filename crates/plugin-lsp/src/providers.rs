//! Generic LSP providers driven by `Language::lsp` rule configuration.
//!
//! Each provider operates on a pre-parsed source module and returns
//! language-agnostic data (`ExportMatch`, etc.) that `LspEngine` formats into
//! host LSP responses.

use handl_host_tooling::lsp::{CompletionItem, DocumentSymbol, Position, Range, TextEdit};
use handl_language::{
    Ast, AstNode, AstRef, LangError, Language, LspDef, LspMatch, LspRule, Span, Token, Tokens,
    Value,
};
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_transform_rewrite::condition::matches_condition;
use handl_plugin_ext_diag::split_statements;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// A parsed source module, ready for LSP rule evaluation.
pub(crate) struct ParsedModule<'a> {
    /// Original source text.
    pub src: &'a str,
    /// All lexer tokens (including trivia), with absolute byte spans.
    pub tokens: Vec<Token>,
    /// Parsed top-level statements, in source order, with absolute spans.
    pub statements: Vec<ParsedStmt>,
}

/// A single parsed top-level statement.
pub(crate) struct ParsedStmt {
    /// Parsed AST arena.
    pub ast: Ast,
}

/// Result of an `export` rule match.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct ExportMatch {
    /// Display name of the exported binding.
    pub name: String,
    /// Concatenated leading/trailing documentation, if any.
    pub docs: Option<String>,
    /// Signature source text, if the rule declares a signature.
    pub signature: Option<String>,
    /// Byte span of the name (for `goto`) or the matched node.
    pub name_span: Span,
}

/// Split `src` into statements, lex, and parse each chunk using the data-driven
/// pipeline.  Trivia tokens are retained in `tokens` for documentation
/// extraction; the Pratt parser filters them internally.
pub(crate) fn parse_module<'a>(
    lang: &Language,
    src: &'a str,
) -> Result<ParsedModule<'a>, LangError> {
    let mut all_tokens = Vec::new();
    let mut statements = Vec::new();

    for chunk in split_statements(src) {
        let toks = lex(lang, &chunk.text)?;
        if toks.as_slice().is_empty() {
            continue;
        }

        // Absolute spans within the original source.
        let abs_tokens: Vec<Token> = toks
            .0
            .into_iter()
            .map(|mut t| {
                t.start += chunk.offset;
                t.end += chunk.offset;
                t
            })
            .collect();

        all_tokens.extend(abs_tokens.iter().cloned());

        let ast = PrattEngine::new().parse(lang, &ParserRequest::new(Tokens(abs_tokens)))?;
        if !ast.is_empty() {
            statements.push(ParsedStmt { ast });
        }
    }

    Ok(ParsedModule {
        src,
        tokens: all_tokens,
        statements,
    })
}

/// Evaluate a single `kind: export` rule against `module` at `pos`.
pub(crate) fn run_export(
    lang: &Language,
    module: &ParsedModule<'_>,
    pos: usize,
    rule: &LspRule,
) -> Option<ExportMatch> {
    let (node_kind, when) = match rule.match_condition.as_ref()? {
        LspMatch::Match { node_kind, when } => (node_kind, when.as_ref()),
        _ => return None,
    };

    // Support `AstShapeDef` ids (e.g. `ast.law`) as well as runtime kinds.
    let target_kind = resolve_node_kind(lang, node_kind);

    // Find the innermost AST node of the requested kind whose span contains
    // `pos` and which satisfies the optional `when` condition.
    let mut best: Option<(&AstNode, &Ast)> = None;
    for stmt in &module.statements {
        for node in stmt.ast.iter() {
            if node.kind != target_kind {
                continue;
            }
            let span = node.span?;
            if span.start > pos || span.end <= pos {
                continue;
            }
            if let Some(cond) = when {
                if !matches_condition(lang, &stmt.ast, node, cond) {
                    continue;
                }
            }
            if let Some((cur, _)) = best {
                let cur_span = cur.span?;
                if (span.end - span.start) < (cur_span.end - cur_span.start) {
                    best = Some((node, &stmt.ast));
                }
            } else {
                best = Some((node, &stmt.ast));
            }
        }
    }

    let (node, ast) = best?;
    let node_span = node.span?;

    let name_cfg = rule.config.get("name")?.as_object()?;
    let dotted = name_cfg
        .get("dotted_last_segment")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let (name, name_span) =
        if let Some(payload_path) = name_cfg.get("payload").and_then(Value::as_str) {
            let n = payload_at(&node.payload, payload_path)
                .and_then(Value::as_str)?
                .to_string();
            let span = token_span_for_name(module, &n, &node_span).unwrap_or(node_span);
            (n, span)
        } else if let Some(role) = name_cfg.get("role").and_then(Value::as_str) {
            let child = child_node(lang, ast, node, role)?;
            let child_span = child.span?;
            let n = source_text(module, child_span).to_string();
            (n, child_span)
        } else {
            return None;
        };

    let display_name = if dotted {
        name.rsplit_once('.')
            .map(|(_, l)| l)
            .unwrap_or(&name)
            .to_string()
    } else {
        name
    };

    let docs = extract_docs(lang, module, &rule.config, node_span);

    let signature = if let Some(sig_cfg) = rule.config.get("signature").and_then(Value::as_object) {
        if let Some(role) = sig_cfg.get("role").and_then(Value::as_str) {
            child_node(lang, ast, node, role)
                .and_then(|c| c.span)
                .map(|s| source_text(module, s).to_string())
        } else if sig_cfg.get("fallback").and_then(Value::as_str) == Some("source") {
            Some(source_text(module, node_span).to_string())
        } else {
            None
        }
    } else {
        None
    };

    Some(ExportMatch {
        name: display_name,
        docs,
        signature,
        name_span,
    })
}

/// Resolve a user-supplied `node_kind` string to a runtime AST kind.
///
/// If the string is an `AstShapeDef` `id` (e.g. `ast.law`), return the
/// corresponding `node_kind`. Otherwise return the string unchanged.
pub(crate) fn resolve_node_kind<'a>(lang: &'a Language, requested: &'a str) -> &'a str {
    if let Some(shape) = lang.ast.iter().find(|s| s.id == requested) {
        return &shape.node_kind;
    }
    requested
}

/// Resolve a named child role to an arena node, using `AstShapeDef` data.
fn child_node<'a>(
    lang: &Language,
    ast: &'a Ast,
    node: &AstNode,
    role: &str,
) -> Option<&'a AstNode> {
    let shape = lang.ast.get_by_identity(&node.kind)?;
    let idx = shape.role_index(role)?;
    let child_ref = node
        .payload
        .get("children")?
        .as_array()?
        .get(idx)?
        .as_u64()? as usize;
    ast.get(AstRef::new(child_ref))
}

/// Navigate a dotted path (`.`-separated keys; numeric segments index arrays)
/// through a JSON value.
fn payload_at<'a>(payload: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(payload);
    }
    let mut cur = payload;
    for seg in path.split('.') {
        cur = if let Some(idx) = seg.parse::<usize>().ok().filter(|_| cur.is_array()) {
            cur.get(idx)?
        } else {
            cur.get(seg)?
        };
    }
    Some(cur)
}

/// Return the span of the token whose text equals `name` and lies inside
/// `search_span`, if one exists.
fn token_span_for_name(module: &ParsedModule<'_>, name: &str, search_span: &Span) -> Option<Span> {
    for t in &module.tokens {
        if t.start >= search_span.start && t.end <= search_span.end && t.text == name {
            return Some(Span {
                start: t.start,
                end: t.end,
            });
        }
    }
    None
}

/// Extract leading and trailing documentation for `node_span` from tokens of the
/// kinds declared in the rule configuration.
fn extract_docs(
    lang: &Language,
    module: &ParsedModule<'_>,
    config: &std::collections::BTreeMap<String, Value>,
    node_span: Span,
) -> Option<String> {
    let docs_cfg = config.get("docs")?.as_object()?;
    let leading_kinds: Vec<String> = docs_cfg
        .get("leading_trivia")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let trailing_kinds: Vec<String> = docs_cfg
        .get("trailing_trivia")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let mut parts = Vec::new();
    parts.extend(tokens_trivia(
        &module.tokens,
        node_span.start,
        Direction::Before,
        &leading_kinds,
    ));
    parts.extend(tokens_trivia(
        &module.tokens,
        node_span.end,
        Direction::After,
        &trailing_kinds,
    ));

    if parts.is_empty() {
        return None;
    }

    Some(
        parts
            .iter()
            .map(|t| clean_token_text(lang, t))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[derive(Clone, Copy)]
enum Direction {
    Before,
    After,
}

/// Collect a contiguous run of trivia tokens immediately before/after a node.
fn tokens_trivia(
    tokens: &[Token],
    boundary: usize,
    direction: Direction,
    kinds: &[String],
) -> Vec<Token> {
    let mut out = Vec::new();
    match direction {
        Direction::Before => {
            let Some(mut i) = tokens
                .iter()
                .rposition(|t| t.end <= boundary && kinds.iter().any(|k| k == &t.kind))
            else {
                return out;
            };
            loop {
                if !kinds.iter().any(|k| k == &tokens[i].kind) {
                    break;
                }
                out.push(tokens[i].clone());
                if i == 0 {
                    break;
                }
                i -= 1;
            }
            out.reverse();
        }
        Direction::After => {
            let Some(mut i) = tokens
                .iter()
                .position(|t| t.start >= boundary && kinds.iter().any(|k| k == &t.kind))
            else {
                return out;
            };
            while i < tokens.len() && kinds.iter().any(|k| k == &tokens[i].kind) {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    out
}

/// Strip common comment/doc delimiters from a trivia token's text.
fn clean_token_text(lang: &Language, token: &Token) -> String {
    let category = lang
        .tokens
        .get_by_identity(&token.kind)
        .and_then(|t| t.category.as_deref());
    let text = token.text.trim();

    if category == Some("block-comment") || (text.starts_with("/*") && text.ends_with("*/")) {
        let inner = &text[2..text.len().saturating_sub(2)];
        return inner.trim().to_string();
    }

    let stripped = text.trim_start_matches(|c: char| c.is_ascii_punctuation() && c != '_');
    if stripped.starts_with(' ') {
        stripped[1..].to_string()
    } else {
        stripped.to_string()
    }
}

/// Slice the source text for a span.
fn source_text<'a>(module: &ParsedModule<'a>, span: Span) -> &'a str {
    &module.src[span.start..span.end]
}

/// Evaluate a single `kind: symbol` rule against `module`.
pub(crate) fn run_symbol(
    lang: &Language,
    module: &ParsedModule<'_>,
    rule: &LspRule,
) -> Vec<DocumentSymbol> {
    let (node_kind, when) = match rule.match_condition.as_ref() {
        Some(LspMatch::Match { node_kind, when }) => (node_kind, when.as_ref()),
        _ => return Vec::new(),
    };

    let target_kind = resolve_node_kind(lang, node_kind);

    let name_cfg = rule.config.get("name").and_then(Value::as_object);
    let dotted = name_cfg
        .and_then(|c| c.get("dotted_last_segment"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let kind = rule
        .config
        .get("symbol_kind")
        .and_then(Value::as_str)
        .and_then(symbol_kind_number)
        .or_else(|| {
            rule.config
                .get("symbol_kind")
                .and_then(Value::as_u64)
                .map(|n| n as u32)
        })
        .unwrap_or(13);

    let mut out = Vec::new();
    for stmt in &module.statements {
        let Some(node) = stmt.ast.nodes().last() else {
            continue;
        };
        if node.kind != target_kind {
            continue;
        }
        if let Some(cond) = when {
            if !matches_condition(lang, &stmt.ast, node, cond) {
                continue;
            }
        }
        let Some(node_span) = node.span else {
            continue;
        };

        let (name, name_span) = match &name_cfg {
            Some(cfg) if cfg.get("payload").and_then(Value::as_str).is_some() => {
                let path = cfg.get("payload").and_then(Value::as_str).unwrap();
                let n = match payload_at(&node.payload, path).and_then(Value::as_str) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                let span = token_span_for_name(module, &n, &node_span).unwrap_or(node_span);
                (n, span)
            }
            Some(cfg) if cfg.get("role").and_then(Value::as_str).is_some() => {
                let role = cfg.get("role").and_then(Value::as_str).unwrap();
                let child = match child_node(lang, &stmt.ast, node, role) {
                    Some(c) => c,
                    None => continue,
                };
                let span = child.span.unwrap_or(node_span);
                let n = source_text(module, span).to_string();
                (n, span)
            }
            _ => continue,
        };

        if name.is_empty() {
            continue;
        }

        let display_name = if dotted {
            name.rsplit_once('.')
                .map(|(_, l)| l)
                .unwrap_or(name.as_str())
                .to_string()
        } else {
            name
        };

        out.push(DocumentSymbol {
            name: display_name,
            detail: None,
            kind,
            range_start: node_span.start,
            range_end: node_span.end,
            selection_start: name_span.start,
            selection_end: name_span.end,
        });
    }
    out
}

/// Convert an LSP `SymbolKind` name (or numeric string) to its numeric value.
pub(crate) fn symbol_kind_number(s: &str) -> Option<u32> {
    let n = match s {
        "File" => 1,
        "Module" => 2,
        "Namespace" => 3,
        "Package" => 4,
        "Class" => 5,
        "Method" => 6,
        "Property" => 7,
        "Field" => 8,
        "Constructor" => 9,
        "Enum" => 10,
        "Interface" => 11,
        "Function" => 12,
        "Variable" => 13,
        "Constant" => 14,
        "String" => 15,
        "Number" => 16,
        "Boolean" => 17,
        "Array" => 18,
        "Object" => 19,
        "Key" => 20,
        "Null" => 21,
        "EnumMember" => 22,
        "Struct" => 23,
        "Event" => 24,
        "Operator" => 25,
        "TypeParameter" => 26,
        _ => s.parse::<u32>().ok()?,
    };
    Some(n)
}

/// Evaluate a single `kind: import_hover` rule against `module` at `pos`.
pub(crate) fn run_import_hover(
    lang: &Language,
    module: &ParsedModule<'_>,
    pos: usize,
    path: &str,
    rule: &LspRule,
) -> Option<String> {
    let (idx, token) = module
        .tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.start <= pos && pos < t.end)?;
    if !is_string_token(token) {
        return None;
    }

    let preceding = rule
        .config
        .get("preceding_tokens")
        .and_then(Value::as_array)
        .map(|arr| as_string_list(arr))
        .unwrap_or_else(|| vec!["token.keyword.from".into(), "token.keyword.import".into()]);

    let prev = module.tokens.get(idx.saturating_sub(1))?;
    if !preceding.iter().any(|k| k == &prev.kind) {
        return None;
    }

    let spec = token
        .text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(&token.text);

    let layouts = module_layouts_patterns(rule);

    let base = base_dir(path)?;
    let target = resolve_module_path(spec, &base, &layouts)?;
    let target_src = fs::read_to_string(&target).ok()?;
    let target_module = parse_module(lang, &target_src).ok()?;

    let docs = module_leading_docs(lang, &target_module, rule)?;
    Some(format!("**module** `{}`\n\n{}", spec, docs))
}

/// Default module layout patterns for `import_hover` / `import_name` when a
/// rule does not declare its own.
const DEFAULT_LAYOUTS: &[&str] = &["{spec}", "{spec}.hnl", "{dir}/{dir}.hnl", "{dir}/mod.hnl"];

/// Read the `module_layouts` field from a rule's config.
///
/// Supports both the flat string form (`["{spec}"]`) and the object form
/// (`[{ pattern: "{spec}" }]`), which is the canonical `lsp.yaml` spelling.
fn module_layouts_patterns(rule: &LspRule) -> Vec<String> {
    rule.config
        .get("module_layouts")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    if let Some(s) = v.as_str() {
                        Some(s.to_string())
                    } else if let Some(obj) = v.as_object() {
                        obj.get("pattern").and_then(Value::as_str).map(String::from)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Expand a layout pattern string using the import `spec`.
///
/// Supported placeholders:
/// - `{spec}`: the import specifier as written.
/// - `{dir}`: in non-final path components, the specifier with any trailing
///   extension stripped; in the final component, the file stem of the specifier.
fn expand_layout_pattern(pattern: &str, spec: &str) -> Option<PathBuf> {
    let spec_path = Path::new(spec);
    let spec_no_ext = spec_path.with_extension("");
    let stem = spec_path.file_stem().and_then(|s| s.to_str())?;

    let mut out = PathBuf::new();
    let components: Vec<&str> = pattern.split('/').collect();
    let last = components.len().saturating_sub(1);

    for (i, comp) in components.iter().enumerate() {
        let is_last = i == last;
        let expanded = if comp.contains("{spec}") {
            comp.replace("{spec}", spec)
        } else if comp.contains("{dir}") {
            let dir = if is_last { stem } else { spec_no_ext.to_str()? };
            comp.replace("{dir}", dir)
        } else {
            (*comp).to_string()
        };
        if !expanded.is_empty() {
            out.push(&expanded);
        }
    }

    Some(out)
}

/// Resolve an import `spec` to an existing file using the configured
/// module layout patterns.
fn resolve_module_path(spec: &str, base: &Path, layouts: &[String]) -> Option<PathBuf> {
    let layouts: Vec<&str> = if layouts.is_empty() {
        DEFAULT_LAYOUTS.to_vec()
    } else {
        layouts.iter().map(|s| s.as_str()).collect()
    };

    for pat in layouts {
        let rel = expand_layout_pattern(pat, spec)?;
        let candidate = base.join(&rel);
        let resolved = candidate
            .canonicalize()
            .ok()
            .unwrap_or_else(|| candidate.clone());
        if resolved.is_file() {
            return Some(resolved);
        }
    }
    None
}

/// Extract leading trivia from the start of `module` as markdown.
fn module_leading_docs(
    lang: &Language,
    module: &ParsedModule<'_>,
    rule: &LspRule,
) -> Option<String> {
    let leading_kinds = rule
        .config
        .get("module_docs")
        .and_then(Value::as_object)
        .and_then(|o| o.get("leading_trivia"))
        .and_then(Value::as_array)
        .map(|arr| as_string_list(arr))
        .unwrap_or_else(|| vec!["token.doc_comment".into()]);

    let mut docs = Vec::new();
    for t in &module.tokens {
        if leading_kinds.iter().any(|k| k == &t.kind) {
            docs.push(clean_token_text(lang, t));
        } else {
            break;
        }
    }
    if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    }
}

/// Whether a token's text is a double-quoted string literal.
fn is_string_token(token: &Token) -> bool {
    token.text.starts_with('"') && token.text.ends_with('"') && token.text.len() >= 2
}

/// Extract a list of strings from a JSON array value.
fn as_string_list(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}

/// Directory portion of an absolute or workspace-relative file path.
fn base_dir(path: &str) -> Option<PathBuf> {
    Path::new(path)
        .parent()
        .and_then(|p| {
            if p.as_os_str().is_empty() {
                std::env::current_dir().ok()
            } else {
                Some(p.to_path_buf())
            }
        })
        .and_then(|p| p.canonicalize().ok().or(Some(p)))
}

/// Convert a byte offset in `src` to an LSP 0-based line/character position.
fn byte_to_position(src: &str, offset: usize) -> Position {
    let clamped = offset.min(src.len());
    let prefix = &src[..clamped];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = (clamped - line_start) as u32;
    Position { line, character }
}

/// Collect exported names from `module` using an `export` rule.
fn collect_export_names(
    lang: &Language,
    module: &ParsedModule<'_>,
    rule: &LspRule,
) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for stmt in &module.statements {
        let Some(node) = stmt.ast.nodes().last() else {
            continue;
        };
        let Some(span) = node.span else {
            continue;
        };
        if let Some(m) = run_export(lang, module, span.start, rule) {
            let detail = m.signature.or(m.docs);
            out.push((m.name, detail));
        }
    }
    out
}

/// Evaluate a single `kind: import_path` completion rule.
pub(crate) fn run_import_path(
    _lang: &Language,
    module: &ParsedModule<'_>,
    pos: usize,
    path: &str,
    rule: &LspRule,
) -> Vec<CompletionItem> {
    let Some((idx, token)) = module
        .tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.start <= pos && pos < t.end)
    else {
        return Vec::new();
    };
    if !is_string_token(token) {
        return Vec::new();
    }

    let preceding = rule
        .config
        .get("preceding_tokens")
        .and_then(Value::as_array)
        .map(|arr| as_string_list(arr))
        .unwrap_or_else(|| vec!["token.keyword.from".into(), "token.keyword.import".into()]);
    let Some(prev) = module.tokens.get(idx.saturating_sub(1)) else {
        return Vec::new();
    };
    if !preceding.iter().any(|k| k == &prev.kind) {
        return Vec::new();
    }

    let spec = token.text[1..token.text.len() - 1].to_string();
    let Some(base) = base_dir(path) else {
        return Vec::new();
    };
    let patterns = module_layouts_patterns(rule);
    if patterns.is_empty() {
        return Vec::new();
    }

    let start = byte_to_position(module.src, token.start);
    let end = byte_to_position(module.src, token.end);
    let range = Range { start, end };

    let mut items = Vec::new();
    let mut seen = BTreeSet::new();

    for pat in &patterns {
        let Some(rel) = expand_layout_pattern(pat, &spec) else {
            continue;
        };
        let candidate = base.join(&rel);
        let resolved = candidate
            .canonicalize()
            .ok()
            .unwrap_or_else(|| candidate.clone());
        if !resolved.exists() || (!resolved.is_dir() && !resolved.is_file()) {
            continue;
        }
        let Some(rel_str) = rel.to_str() else {
            continue;
        };
        let rel_str = rel_str.to_string();
        if !seen.insert(rel_str.clone()) {
            continue;
        }
        let new_text = format!("\"{}\"", rel_str);
        let mut item = CompletionItem::new(&rel_str);
        item.detail = resolved.is_dir().then_some("directory".to_string());
        item.text_edit = Some(TextEdit {
            range: range.clone(),
            new_text,
        });
        items.push(item);
    }

    items
}

/// Evaluate a single `kind: import_name` completion rule.
pub(crate) fn run_import_name(
    lang: &Language,
    module: &ParsedModule<'_>,
    def: &LspDef,
    pos: usize,
    path: &str,
    rule: &LspRule,
) -> Vec<CompletionItem> {
    let name_tokens: Vec<String> = rule
        .config
        .get("name_tokens")
        .and_then(Value::as_array)
        .map(|arr| as_string_list(arr))
        .unwrap_or_else(|| vec!["token.keyword.import".into()]);

    let Some((idx, _)) = module
        .tokens
        .iter()
        .enumerate()
        .rfind(|(_, t)| t.start <= pos)
    else {
        return Vec::new();
    };

    let import_idx = if name_tokens.iter().any(|k| k == &module.tokens[idx].kind) {
        idx
    } else if idx > 0
        && name_tokens
            .iter()
            .any(|k| k == &module.tokens[idx - 1].kind)
    {
        idx - 1
    } else {
        return Vec::new();
    };

    let Some(spec_tok) = module
        .tokens
        .get(import_idx.saturating_sub(1))
        .filter(|t| is_string_token(t))
    else {
        return Vec::new();
    };

    let preceding = rule
        .config
        .get("preceding_tokens")
        .and_then(Value::as_array)
        .map(|arr| as_string_list(arr))
        .unwrap_or_else(|| vec!["token.keyword.from".into(), "token.keyword.import".into()]);
    if let Some(prev) = module.tokens.get(import_idx.saturating_sub(2)) {
        if !preceding.iter().any(|k| k == &prev.kind) {
            return Vec::new();
        }
    }

    let spec = &spec_tok.text[1..spec_tok.text.len() - 1];
    let Some(base) = base_dir(path) else {
        return Vec::new();
    };
    let layouts = module_layouts_patterns(rule);
    let Some(target) = resolve_module_path(spec, &base, &layouts) else {
        return Vec::new();
    };
    let Ok(target_src) = fs::read_to_string(&target) else {
        return Vec::new();
    };
    let Ok(target_module) = parse_module(lang, &target_src) else {
        return Vec::new();
    };

    let export_rules: Vec<String> = rule
        .config
        .get("export_rules")
        .and_then(Value::as_array)
        .map(|arr| as_string_list(arr))
        .unwrap_or_else(|| {
            def.rules
                .iter()
                .filter_map(|(id, r)| {
                    if r.kind == "export" {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        });

    let mut items = Vec::new();
    let mut seen = BTreeSet::new();

    for rule_id in &export_rules {
        let Some(export_rule) = def.rules.get(rule_id.as_str()) else {
            continue;
        };
        if export_rule.kind != "export" {
            continue;
        }
        for (name, detail) in collect_export_names(lang, &target_module, export_rule) {
            if !seen.insert(name.clone()) {
                continue;
            }
            let mut item = CompletionItem::new(&name);
            item.detail = detail;
            items.push(item);
        }
    }

    items
}

/// Evaluate a single `kind: export_name` completion rule.
pub(crate) fn run_exported_names(
    lang: &Language,
    module: &ParsedModule<'_>,
    def: &LspDef,
    rule: &LspRule,
) -> Vec<CompletionItem> {
    let export_rules: Vec<String> = rule
        .config
        .get("export_rules")
        .and_then(Value::as_array)
        .map(|arr| as_string_list(arr))
        .unwrap_or_else(|| {
            def.rules
                .iter()
                .filter_map(|(id, r)| {
                    if r.kind == "export" {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        });

    let mut items = Vec::new();
    let mut seen = BTreeSet::new();

    for rule_id in &export_rules {
        let Some(export_rule) = def.rules.get(rule_id.as_str()) else {
            continue;
        };
        if export_rule.kind != "export" {
            continue;
        }
        for (name, detail) in collect_export_names(lang, module, export_rule) {
            if !seen.insert(name.clone()) {
                continue;
            }
            let mut item = CompletionItem::new(&name);
            item.detail = detail;
            items.push(item);
        }
    }

    items
}
