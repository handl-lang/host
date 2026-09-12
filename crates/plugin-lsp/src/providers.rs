//! Generic LSP providers driven by `Language::lsp` rule configuration.
//!
//! Each provider operates on a pre-parsed source module and returns
//! language-agnostic data (`ExportMatch`, etc.) that `LspEngine` formats into
//! host LSP responses.

use handl_language::{
    Ast, AstNode, AstRef, LangError, Language, LspMatch, LspRule, Span, Token, Tokens, Value,
};
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_transform_rewrite::condition::matches_condition;
use handl_plugin_ext_diag::split_statements;

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
fn resolve_node_kind<'a>(lang: &'a Language, requested: &'a str) -> &'a str {
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
