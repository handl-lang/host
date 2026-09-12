//! `textDocument/hover` plugin for the handl LSP server.
//!
//! Provides token-level markdown hover text for canonical `.hnl` source.
//! Cross-file arrow refinement hovers stay in the host (`handl-host-lsp`).

use handl_host_tooling::lsp::{HoverPlugin, HoverRequest, HoverResponse};
use handl_kit_canonical::{Kit, SpannedToken, TokenClasses};
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
        Ok(HoverResponse(token_hover(&request.source, request.pos)))
    }
}

fn token_hover(source: &str, pos: usize) -> Option<String> {
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
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
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
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
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
