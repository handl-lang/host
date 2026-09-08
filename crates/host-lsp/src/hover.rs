use std::path::Path;

use serde_json::{Value, json};

use handl_kit_canonical::{
    SpannedToken, is_bool_kind, is_comment_kind, is_delimiter_kind, is_doc_comment_kind,
    is_identifier_kind, is_keyword_kind, is_null_kind, is_number_kind, is_operator_kind,
    is_string_kind, is_template_kind,
};
use handl_language::{Language, TokenCategory as TC};

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;

pub fn hover_at(source: &str, offset: usize) -> Option<Value> {
    hover_at_with_path(source, offset, None)
}

/// If the cursor is on an identifier preceded by `.` (a property access),
/// return the property name. Otherwise return `None`.
pub fn property_at_offset(source: &str, offset: usize) -> Option<String> {
    let kit = kit_for_file(None).ok()?;
    let lang = kit.lang();
    let tokens = kit.lex_with_spans(source).ok()?;

    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.span.start <= offset && offset < t.span.end)?;

    if !is_identifier_kind(lang, token.kind.as_str()) {
        return None;
    }

    // Check if preceded by `.`
    if index > 0 && lang.token_is(&tokens[index - 1].kind, TC::Dot) {
        return Some(token.text.clone());
    }

    None
}

pub fn hover_at_with_path(source: &str, offset: usize, file_path: Option<&str>) -> Option<Value> {
    let kit = kit_for_file(file_path).ok()?;
    let lang = kit.lang();
    let tokens = kit.lex_with_spans(source).ok()?;

    // Find the token at the cursor position.
    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.span.start <= offset && offset < t.span.end)?;

    // Try doc-comment-aware hover first (cross-file arrow docs).
    if let Some(md) = doc_comment_hover(lang, &tokens, index, token, source, file_path) {
        let (start_line, start_char) = offset_to_position(source, token.span.start);
        let (end_line, end_char) = offset_to_position(source, token.span.end);
        return Some(json!({
            "contents": { "kind": "markdown", "value": md },
            "range": {
                "start": { "line": start_line, "character": start_char },
                "end": { "line": end_line, "character": end_char },
            }
        }));
    }

    let markdown = classify_token(lang, &tokens, index, token).or_else(|| {
        // Fallback: if we can't classify the token but the file has doc
        // comments, show those instead.
        extract_doc_comments(source)
    })?;

    let (start_line, start_char) = offset_to_position(source, token.span.start);
    let (end_line, end_char) = offset_to_position(source, token.span.end);

    Some(json!({
        "contents": { "kind": "markdown", "value": markdown },
        "range": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": end_line, "character": end_char },
        }
    }))
}

/// Extract the leading `---` doc comments from a source file as markdown text.
///
/// Strips the `---` prefix and one optional leading space from each line.
/// Returns `None` if there are no doc comments at the top of the file.
pub fn extract_doc_comments(source: &str) -> Option<String> {
    let kit = kit_for_file(None).ok()?;
    let lang = kit.lang();
    let tokens = kit.lex_with_spans(source).ok()?;

    let mut doc_lines: Vec<String> = Vec::new();
    for token in &tokens {
        if is_doc_comment_kind(lang, token.kind.as_str()) {
            // Strip the `---` prefix and one optional leading space.
            let text = token.text.strip_prefix("---").unwrap_or(&token.text);
            let text = text.strip_prefix(' ').unwrap_or(text);
            doc_lines.push(text.to_string());
        } else if is_comment_kind(lang, token.kind.as_str()) {
            // Skip regular comments — they're not doc comments.
            continue;
        } else {
            // First non-comment token — stop collecting.
            break;
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

/// Try to produce a hover based on doc comments:
/// 1. If hovering over an identifier that matches an arrow file name, read
///    that file's doc comments and display them.
/// 2. If hovering over the `rec` keyword in an arrow file, show the current
///    file's own doc comments.
fn doc_comment_hover(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
    token: &SpannedToken,
    source: &str,
    file_path: Option<&str>,
) -> Option<String> {
    // Case 2: hovering over `rec` or `while` keyword → show this file's doc comments.
    if is_keyword_kind(lang, token.kind.as_str())
        && (token.text == "rec" || token.text == "while")
    {
        if let Some(docs) = extract_doc_comments(source) {
            return Some(format_doc_comment_hover(&docs, &token.text));
        }
    }

    // Case 1: hovering over an identifier that matches an arrow file name.
    if !is_identifier_kind(lang, token.kind.as_str()) {
        return None;
    }

    // Skip if this is a property access (preceded by `.`) — those are fields,
    // not arrow references.
    if index > 0 && lang.token_is(&tokens[index - 1].kind, TC::Dot) {
        return None;
    }

    let name = token.text.as_str();

    // Find the arrows root from the file path.
    let path = file_path?;
    let arrows_root = crate::refinement::find_arrows_root(path)?;

    // Search for `name.hnl` under the arrows root.
    let target = find_arrow_file(&arrows_root, name)?;
    let target_source = std::fs::read_to_string(&target).ok()?;
    let docs = extract_doc_comments(&target_source)?;

    Some(format_doc_comment_hover(&docs, name))
}

/// Format doc comments as a markdown hover string.
fn format_doc_comment_hover(docs: &str, name: &str) -> String {
    format!("**`{}`**\n\n---\n\n{}", name, docs)
}

/// Try to produce a hover for an identifier that references another arrow
/// file. Shows the referenced arrow's doc comments + inferred refinement
/// contract.
pub fn cross_arrow_hover(
    source: &str,
    offset: usize,
    file_path: Option<&str>,
    refinement: &crate::refinement::RefinementEngine,
) -> Option<String> {
    let kit = kit_for_file(file_path).ok()?;
    let lang = kit.lang();
    let tokens = kit.lex_with_spans(source).ok()?;

    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.span.start <= offset && offset < t.span.end)?;

    if !is_identifier_kind(lang, token.kind.as_str()) {
        return None;
    }

    // Skip property access (preceded by `.`).
    if index > 0 && lang.token_is(&tokens[index - 1].kind, TC::Dot) {
        return None;
    }

    let name = token.text.as_str();

    // Find the arrows root and the referenced arrow file.
    let path = file_path?;
    let arrows_root = crate::refinement::find_arrows_root(path)?;
    let target = find_arrow_file(&arrows_root, name)?;
    let target_source = std::fs::read_to_string(&target).ok()?;

    let mut md = String::new();

    // Add doc comments from the referenced arrow file.
    if let Some(docs) = extract_doc_comments(&target_source) {
        md.push_str(&format!("**`{}`**\n\n---\n\n{}\n\n", name, docs));
    }

    // Add the inferred refinement contract for the referenced arrow.
    if let Some(Ok(result)) = refinement.infer_document(&target_source, Some(target.to_str()?)) {
        md.push_str("### Inferred Contract\n\n");
        md.push_str(&format!("**Input**  `{}`\n\n", result.input.pretty()));
        md.push_str(&format!("**Output** `{}`\n\n", result.output.pretty()));
        md.push_str(&format!(
            "**Guarantee** `{}`\n",
            result.guarantee.simplify().pretty()
        ));
    }

    if md.is_empty() { None } else { Some(md) }
}

/// Recursively search for `name.hnl` under `root`.
fn find_arrow_file(root: &str, name: &str) -> Option<std::path::PathBuf> {
    let target_name = format!("{}.hnl", name);
    search_dir(Path::new(root), &target_name)
}

fn search_dir(dir: &Path, file_name: &str) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().map(|n| n == "contracts").unwrap_or(false) {
                continue;
            }
            if let Some(found) = search_dir(&path, file_name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(file_name) {
            return Some(path);
        }
    }
    None
}

fn classify_token(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
    token: &SpannedToken,
) -> Option<String> {
    let kind = token.kind.as_str();

    // Brackets / braces / parens — context-aware (classified via
    // `lang.tokens` categories, ADR-0081).
    match lang.token_category(kind) {
        Some(TC::OpenBracket) | Some(TC::CloseBracket) => {
            return Some(bracket_hover(lang, tokens, index, token, "square"));
        }
        Some(TC::OpenBrace) | Some(TC::CloseBrace) => {
            return Some(bracket_hover(lang, tokens, index, token, "curly"));
        }
        Some(TC::OpenParen) | Some(TC::CloseParen) => {
            return Some(bracket_hover(lang, tokens, index, token, "paren"));
        }
        Some(TC::Ellipsis) => {
            return Some(spread_hover(lang, tokens, index, token));
        }
        Some(TC::Dot) => {
            return Some(dot_hover(lang, tokens, index, token));
        }
        _ => {}
    }

    if is_number_kind(lang, kind) {
        Some(format!("**number** `{}`", token.text))
    } else if is_string_kind(lang, kind) {
        let s = &token.text;
        let inner = &s[1..s.len().saturating_sub(1)];
        Some(format!("**string** `{}`", escape_string(inner)))
    } else if is_bool_kind(lang, kind) {
        Some(format!("**boolean** `{}`", token.text))
    } else if is_null_kind(lang, kind) {
        Some("**null** literal".to_string())
    } else if is_keyword_kind(lang, kind) {
        Some(keyword_hover(&token.text))
    } else if is_identifier_kind(lang, kind) {
        // Arrow combinator calls get specialized hovers.
        if let Some(md) = arrow_combinator_hover(lang, tokens, index, token) {
            return Some(md);
        }
        Some(identifier_hover(lang, tokens, index, token))
    } else if is_operator_kind(lang, kind) {
        Some(operator_hover(&token.text))
    } else if is_delimiter_kind(lang, kind) {
        Some(format!("**punctuation** `{}`", token.text))
    } else if is_doc_comment_kind(lang, kind) {
        Some("**doc comment**".to_string())
    } else if is_comment_kind(lang, kind) {
        Some("**comment**".to_string())
    } else if is_template_kind(lang, kind) {
        Some("**template string**".to_string())
    } else {
        None
    }
}

/// If the identifier is an arrow combinator call (followed by `(`), return
/// a specialized hover with documentation and signature. Otherwise `None`.
fn arrow_combinator_hover(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
    token: &SpannedToken,
) -> Option<String> {
    let is_kind = |i: usize, c: TC| -> bool {
        tokens.get(i).map(|t| lang.token_is(&t.kind, c)).unwrap_or(false)
    };

    // Must be a call: followed by `(`. Exception: `id` is used without parens.
    let is_call = is_kind(index + 1, TC::OpenParen);
    if !is_call && token.text != "id" {
        return None;
    }

    let name = token.text.as_str();
    let (title, doc, sig) = match name {
        "arr" => (
            "Arrow combinator: `arr` (kernel)",
            "Lift a named pure function into the arrow. Kernel concept — in v0.1 surface syntax, named arrows are used directly (e.g. `name args`).",
            "arr(\"op\", args…)  —  kernel form; surface: `name args`",
        ),
        "comp" | "seq" => (
            "Arrow combinator: `comp` (sequential composition)",
            "Compose two arrows: the output of `a` feeds the input of `b`. Written as `a >>> b` in v0.1 surface syntax.",
            "comp(a, b)  —  kernel form; surface: `a >>> b`",
        ),
        "first" => (
            "Arrow combinator: `first` (kernel)",
            "Run `inner` on the first component of a pair, passing the second through unchanged. Kernel concept for fan-out / parallel wiring.",
            "first(inner)  —  (a, b) ~> (f(a), b)",
        ),
        "choice" => (
            "Arrow combinator: `choice`",
            "Route on `Either`: `l` handles `Left`, `r` handles `Right`. ArrowChoice — conditional branching. Written as `l ||| r` in v0.1 surface syntax.",
            "choice(l, r)  —  kernel form; surface: `l ||| r`",
        ),
        "app" => (
            "Arrow combinator: `app` (kernel)",
            "Dynamic / higher-order arrow application. Kernel concept — given `f : A ~> B` and `x : A`, `app(f, x)` invokes `f` on `x`.",
            "app(\"name\", args…)  —  (A ~> B, A) ~> B",
        ),
        "loop" => (
            "Arrow combinator: `loop` (kernel)",
            "Feedback channel (NOT iteration). `body` has shape `(In, State) ~> (Out, State)`; `Loop` hides `State`, exposing `In ~> Out`. Kernel concept — v0.1 surface uses `rec` (feedback loop) or `while`/`for` (derived patterns).",
            "loop { state }( body )  —  kernel; surface: `rec { … }`",
        ),
        "id" => (
            "Arrow combinator: `id`",
            "Identity arrow — passes input through unchanged.",
            "id  —  A ~> A",
        ),
        "insert" => (
            "Arrow: `insert`",
            "Insert an element into a sorted list, maintaining sorted order. Used as a building block for `sort`.",
            "insert({ elem, acc })  —  Sorted(acc)",
        ),
        _ => return None,
    };

    Some(format!("**{}**\n\n{}\n\n```\n{}\n```", title, doc, sig))
}

/// Hover for brackets/braces/parens, showing what they delimit.
fn bracket_hover(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
    token: &SpannedToken,
    bracket_type: &str,
) -> String {
    let (label, open_cat, close_cat) = match bracket_type {
        "square" => ("Array / list", TC::OpenBracket, TC::CloseBracket),
        "curly" => ("Record / object", TC::OpenBrace, TC::CloseBrace),
        "paren" => ("Group / call", TC::OpenParen, TC::CloseParen),
        _ => ("Group", TC::OpenParen, TC::CloseParen),
    };

    // Count elements inside the brackets for opening brackets.
    if lang.token_is(&token.kind, open_cat) {
        let (count, kind_desc) =
            count_bracket_contents(lang, tokens, index, open_cat, close_cat);
        if count > 0 {
            return format!("**{}** — {} {}", label, count, kind_desc);
        }
        if lang.token_is(&token.kind, TC::OpenBracket) {
            return format!("**{}** — empty list `[]`", label);
        }
        if lang.token_is(&token.kind, TC::OpenBrace) {
            return format!("**{}** — empty record `{{}}`", label);
        }
        return format!("**{}**", label);
    }
    format!("**{}** (close)", label)
}

/// Count the top-level elements between an opening and closing bracket.
fn count_bracket_contents(
    lang: &Language,
    tokens: &[SpannedToken],
    open_index: usize,
    open_cat: TC,
    close_cat: TC,
) -> (usize, &'static str) {
    let mut depth = 1;
    let mut commas = 0;
    let mut has_content = false;
    let mut i = open_index + 1;
    while i < tokens.len() && depth > 0 {
        let k = tokens[i].kind.as_str();
        if lang.token_is(k, open_cat) {
            depth += 1;
        } else if lang.token_is(k, close_cat) {
            depth -= 1;
            if depth == 0 {
                break;
            }
        } else if depth == 1 && lang.token_is(k, TC::Comma) {
            commas += 1;
        } else if depth == 1 && !is_comment_kind(lang, k) {
            has_content = true;
        }
        i += 1;
    }
    let count = if has_content { commas + 1 } else { 0 };
    let desc = if open_cat == TC::OpenBrace {
        "fields"
    } else if open_cat == TC::OpenBracket {
        "elements"
    } else {
        "arguments"
    };
    (count, desc)
}

/// Hover for the spread/rest operator `...`.
fn spread_hover(
    lang: &Language,
    tokens: &[SpannedToken],
    _index: usize,
    _token: &SpannedToken,
) -> String {
    // Check context: inside brackets → spread, inside pattern → rest
    let in_brackets = tokens
        .iter()
        .take(_index)
        .rev()
        .take_while(|t| {
            !lang.token_is(&t.kind, TC::OpenBracket)
                && !lang.token_is(&t.kind, TC::CloseBracket)
                && !lang.token_is(&t.kind, TC::OpenBrace)
                && !lang.token_is(&t.kind, TC::CloseBrace)
        })
        .any(|t| {
            lang.token_is(&t.kind, TC::OpenBracket) || lang.token_is(&t.kind, TC::OpenBrace)
        });

    if in_brackets {
        "**Spread / rest** `...`\n\nSpreads elements into an array, or collects remaining elements in a pattern.".to_string()
    } else {
        "**Spread / rest** `...`\n\nSpreads or collects remaining items.".to_string()
    }
}

/// Hover for the dot accessor `.`.
fn dot_hover(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
    _token: &SpannedToken,
) -> String {
    // Check what follows: `ident` → property access, `ident(` → method call
    let next_is_ident = tokens
        .get(index + 1)
        .map(|t| is_identifier_kind(lang, &t.kind))
        .unwrap_or(false);
    let next_is_call = tokens
        .get(index + 2)
        .map(|t| lang.token_is(&t.kind, TC::OpenParen))
        .unwrap_or(false);

    if next_is_ident && next_is_call {
        return "**Method call** `.`\n\nAccesses a member and calls it.".to_string();
    }
    if next_is_ident {
        return "**Property access** `.`\n\nAccesses a field or member.".to_string();
    }
    "**Accessor** `.`".to_string()
}

/// Produce a granular hover for a keyword, with documentation and syntax
/// hint. Dispatches on the keyword's lexeme text (the keyword's surface
/// spelling), not the token kind id (ADR-0081).
fn keyword_hover(text: &str) -> String {
    let (synopsis, syntax) = match text {
        "match" => (
            "Pattern-match on a value",
            "match `expr` { `pat` { `body` }, … }",
        ),
        "if" => ("Conditional expression", "if `cond` { `a` } else { `b` }"),
        "else" => ("Else-branch of `if`", "else { `expr` }"),
        "rec" => (
            "Feedback loop — arrow `Loop` combinator in surface syntax",
            "rec { `state` } { `body` }",
        ),
        "while" => (
            "While loop — iterate while a condition holds",
            "while `cond` { `body` }",
        ),
        "for" => (
            "For loop — iterate over a collection",
            "for `x` in `coll` { `body` }",
        ),
        "do" => ("Do-while loop body", "do { `body` } while `cond`"),
        "in" => (
            "Iterator binding — used with `for`",
            "for `x` in `coll` { `body` }",
        ),
        "return" => ("Return a value from an arrow", "return `expr`"),
        "import" => ("Import a module / arrow", "import `name` from `module`"),
        "from" => ("Module source for an import", "import `name` from `module`"),
        "infer" => ("Infer a type or contract", "infer `name` : `signature`"),
        "type" => ("Type declaration", "type `Name` = `definition`"),
        "trait" => (
            "Trait declaration — a set of laws / operations",
            "trait `Name` { `members` }",
        ),
        "impl" => (
            "Implementation of a trait",
            "impl `Trait` for `Type` { `members` }",
        ),
        "law" => (
            "Law declaration — assert a contract signature",
            "law `name` : `signature`",
        ),
        "true" => ("Boolean literal", "true"),
        "false" => ("Boolean literal", "false"),
        _ => return format!("`keyword` — `{}`", text),
    };
    format!("**{}** `{}`\n\n```\n{}\n```", synopsis, text, syntax)
}

/// Produce a granular hover for an identifier, showing its syntactic role
/// (binding, property, parameter, function, type) and usage count.
fn identifier_hover(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
    token: &SpannedToken,
) -> String {
    let role = classify_identifier_role(lang, tokens, index);
    let usage_count = tokens
        .iter()
        .filter(|t| is_identifier_kind(lang, &t.kind) && t.text == token.text)
        .count();

    let mut md = format!("**{}** `{}`", role.label, token.text);

    if let Some(detail) = role.detail {
        md.push_str(&format!("\n\n{}", detail));
    }

    if usage_count > 1 {
        md.push_str(&format!("\n\n_{} occurrences in this file_", usage_count));
    }

    md
}

/// Produce a hover for an operator, with its name.
fn operator_hover(text: &str) -> String {
    let name = match text {
        "=" => "equality",
        "==" => "equality",
        "::" => "binding",
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
        "~>" => "arrow operator (function definition / type signature)",
        ">>>" => "sequential composition",
        "***" => "parallel product",
        "&&&" => "fan-out",
        "|||" => "choice",
        "??" => "nullish coalescing",
        "?." => "optional chaining",
        ":" => "type annotation / key separator",
        "." => "field access",
        "..." => "spread / rest",
        _ => "operator",
    };
    format!("**{}** `{}`", name, text)
}

// ---------------------------------------------------------------------------
// Identifier role classification (shared logic with completion.rs)
// ---------------------------------------------------------------------------

struct IdentifierRole {
    label: &'static str,
    detail: Option<String>,
}

fn classify_identifier_role(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
) -> IdentifierRole {
    let is_kind = |i: usize, c: TC| -> bool {
        tokens.get(i).map(|t| lang.token_is(&t.kind, c)).unwrap_or(false)
    };

    let ident = tokens[index].text.as_str();

    // Property access: preceded by `.` → this is a member/field.
    // e.g. `state.acc`, `state.remaining`, `input.0`
    if index > 0 && is_kind(index - 1, TC::Dot) {
        // If followed by `(`, it's a method call: `state.f(h)`, `arr.map(...)`
        if is_kind(index + 1, TC::OpenParen) {
            return IdentifierRole {
                label: "method",
                detail: Some(format!("Called via `.` — method on the receiver")),
            };
        }
        return IdentifierRole {
            label: "property",
            detail: Some("Accessed via `.` — a field or member".to_string()),
        };
    }

    // PascalCase → type
    if ident
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
        && ident.chars().count() > 1
        && !ident.contains('.')
    {
        return IdentifierRole {
            label: "type",
            detail: Some(format!("PascalCase identifier — inferred as a type name")),
        };
    }

    // `type.foo` → type reference
    if ident == "type" && is_kind(index + 1, TC::Dot) {
        return IdentifierRole {
            label: "type reference",
            detail: None,
        };
    }

    // Function call: next token is `(`
    if is_kind(index + 1, TC::OpenParen) {
        return IdentifierRole {
            label: "function",
            detail: Some(format!("Called with `(` — function or callable")),
        };
    }

    // Dotted call: `ident . ident (`
    if is_kind(index + 1, TC::Dot)
        && tokens
            .get(index + 2)
            .map(|t| is_identifier_kind(lang, &t.kind))
            .unwrap_or(false)
        && is_kind(index + 3, TC::OpenParen)
    {
        let callee = tokens
            .get(index + 2)
            .map(|t| t.text.as_str())
            .unwrap_or("?");
        return IdentifierRole {
            label: "function (qualified)",
            detail: Some(format!("Module for `{}.{}(…)`", ident, callee)),
        };
    }

    // `key:` inside parens → parameter; inside braces → property
    if is_kind(index + 1, TC::Colon) {
        return match enclosing_open_delim(lang, tokens, index) {
            Some(TC::OpenParen) => IdentifierRole {
                label: "parameter",
                detail: Some("Named field in `(…)` — a parameter".to_string()),
            },
            Some(TC::OpenBrace) => IdentifierRole {
                label: "property",
                detail: Some("Named field in `{ … }` — a record property".to_string()),
            },
            _ => IdentifierRole {
                label: "binding",
                detail: Some("Followed by `:` — a binding declaration".to_string()),
            },
        };
    }

    // Binding declaration: followed by `::`
    if is_kind(index + 1, TC::Bind) {
        return IdentifierRole {
            label: "binding",
            detail: Some("Followed by `::` — a binding declaration".to_string()),
        };
    }

    // Dotted access: `x.y` — x is a value/module, y is a field/member
    if is_kind(index + 1, TC::Dot) {
        return IdentifierRole {
            label: "value",
            detail: Some("Followed by `.` — a value or module".to_string()),
        };
    }

    IdentifierRole {
        label: "variable",
        detail: None,
    }
}

/// Find the category of the nearest unmatched opening delimiter enclosing
/// `index` (paren or brace tracked; other bracket categories are ignored,
/// matching the original two-counter scan).
fn enclosing_open_delim(
    lang: &Language,
    tokens: &[SpannedToken],
    index: usize,
) -> Option<TC> {
    let mut depth_paren: i32 = 0;
    let mut depth_brace: i32 = 0;
    let mut i = index;
    while i > 0 {
        i -= 1;
        match lang.token_category(tokens[i].kind.as_str()) {
            Some(TC::CloseParen) => depth_paren += 1,
            Some(TC::CloseBrace) => depth_brace += 1,
            Some(TC::OpenParen) if depth_paren == 0 => return Some(TC::OpenParen),
            Some(TC::OpenParen) => depth_paren -= 1,
            Some(TC::OpenBrace) if depth_brace == 0 => return Some(TC::OpenBrace),
            Some(TC::OpenBrace) => depth_brace -= 1,
            _ => {}
        }
    }
    None
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
