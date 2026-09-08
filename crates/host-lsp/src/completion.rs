use std::collections::BTreeMap;

use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, is_identifier_kind, is_keyword_kind};
use handl_language::{Language, TokenCategory as TC};

use crate::kit_resolver::kit_for_file;

/// LSP CompletionItemKind values.
const KIND_KEYWORD: i64 = 14;
const KIND_SNIPPET: i64 = 15;
const KIND_VARIABLE: i64 = 6;
const KIND_FUNCTION: i64 = 3;
const KIND_CLASS: i64 = 7;
const KIND_PROPERTY: i64 = 8;
const KIND_PARAMETER: i64 = 6;
const KIND_CONSTANT: i64 = 21;

/// A keyword entry: `(text, label, snippet_body)`.
///
/// `snippet_body` is the body of a VS Code snippet (use `${1:placeholder}`
/// syntax for tab-stops). If `None`, the keyword is inserted as-is.
struct KeywordEntry {
    text: &'static str,
    detail: &'static str,
    snippet: Option<&'static str>,
}

/// All handl keywords, with optional snippet bodies for structural forms.
const KEYWORDS: &[KeywordEntry] = &[
    KeywordEntry {
        text: "match",
        detail: "keyword: match expression",
        snippet: Some("match ${1:expr} {\n  ${2:pat}: ${3:body}\n}"),
    },
    KeywordEntry {
        text: "if",
        detail: "keyword: conditional",
        snippet: Some("if ${1:cond} {\n  ${2:then}\n} else {\n  ${3:else}\n}"),
    },
    KeywordEntry {
        text: "else",
        detail: "keyword: else-branch of if",
        snippet: None,
    },
    KeywordEntry {
        text: "rec",
        detail: "keyword: rec block (feedback loop)",
        snippet: Some("rec {\n  ${1:// state bindings}\n  ${2:// body}\n}"),
    },
    KeywordEntry {
        text: "while",
        detail: "keyword: while loop",
        snippet: Some("while ${1:cond} {\n  ${2:// body}\n}"),
    },
    KeywordEntry {
        text: "do",
        detail: "keyword: do-while loop",
        snippet: Some("do {\n  ${1:// body}\n} while ${2:cond}"),
    },
    KeywordEntry {
        text: "for",
        detail: "keyword: for loop",
        snippet: Some("for ${1:x} in ${2:xs} {\n  ${3:// body}\n}"),
    },
    KeywordEntry {
        text: "in",
        detail: "keyword: for-in iterator",
        snippet: None,
    },
    KeywordEntry {
        text: "return",
        detail: "keyword: return from rec block",
        snippet: Some("return ${1:expr}"),
    },
    KeywordEntry {
        text: "import",
        detail: "keyword: module import",
        snippet: Some("import ${1:name} from \"${2:path}\""),
    },
    KeywordEntry {
        text: "from",
        detail: "keyword: import source",
        snippet: None,
    },
    KeywordEntry {
        text: "infer",
        detail: "keyword: type inference directive",
        snippet: Some("infer ${1:name}"),
    },
    KeywordEntry {
        text: "type",
        detail: "keyword: type namespace",
        snippet: None,
    },
    KeywordEntry {
        text: "trait",
        detail: "keyword: trait declaration",
        snippet: Some("trait ${1:name} {\n  ${2:// laws}\n}"),
    },
    KeywordEntry {
        text: "impl",
        detail: "keyword: trait implementation",
        snippet: Some("impl ${1:trait} {\n  ${2:// methods}\n}"),
    },
    KeywordEntry {
        text: "law",
        detail: "keyword: law declaration (replaces axiom)",
        snippet: Some("law ${1:name} : ${2:Type}\n  guarantee: ${3:True}"),
    },
    KeywordEntry {
        text: "true",
        detail: "boolean literal",
        snippet: None,
    },
    KeywordEntry {
        text: "false",
        detail: "boolean literal",
        snippet: None,
    },
    KeywordEntry {
        text: "null",
        detail: "null literal",
        snippet: None,
    },
];

/// Arrow combinator snippets — useful inside arrow-graph expressions.
const ARROW_SNIPPETS: &[(&str, &str, &str)] = &[
    (
        ">>>",
        "arrow: a >>> b — sequential composition",
        "${1:a} >>> ${2:b}",
    ),
    (
        "***",
        "arrow: a *** b — parallel product",
        "${1:a} *** ${2:b}",
    ),
    ("&&&", "arrow: a &&& b — fan-out", "${1:a} &&& ${2:b}"),
    ("|||", "arrow: a ||| b — choice", "${1:a} ||| ${2:b}"),
    (
        "~>",
        "arrow: pat ~> body — arrow definition",
        "${1:pat} ~> ${2:body}",
    ),
];

/// Snippet templates moved from the VS Code extension's static snippets file
/// (`snippets/hnl.code-snippets.json`) into the server's completion provider
/// (ADR 0002, Step 2). Each entry is `(trigger, detail, body)`.
///
/// `body` uses VS Code snippet syntax (`$1`, `${1:placeholder}`, …) and is
/// emitted with `insertTextFormat = 2` (Snippet), the same format used by both
/// static snippets and LSP completion items.
///
/// Only triggers that do not collide with the keyword completions in
/// [`KEYWORDS`] are listed here. Keyword-triggered templates (`match`, `if`,
/// `rec`, `while`, `for`, `return`, `import`, `infer`, `law`) are already
/// provided by [`KEYWORDS`] with their own snippet bodies, so they are not
/// duplicated here. This keeps the full set of trigger names from the original
/// static file available while avoiding duplicate completion items.
const SNIPPETS: &[(&str, &str, &str)] = &[
    (
        "~>",
        "An arrow: pattern ~> body",
        "${1:pattern} ~> ${2:body}",
    ),
    (
        "~>args",
        "A multi-arg arrow: [args] ~> body (no currying, ID.2)",
        "[${1:args}] ~> ${2:body}",
    ),
    (
        "rec-match",
        "A rec block with match on a list (common fold pattern)",
        "rec {\n  remaining = ${1:input},\n  acc = ${2:[]},\n  match remaining {\n    []: return acc,\n    [h, ...t]: {\n      ${3:// process h}\n      remaining = t,\n      acc = ${4:// update acc}\n    }\n  }\n}",
    ),
    (
        "do-while",
        "A do-while loop: do { body } while cond",
        "do {\n  ${1:// body}\n} while ${2:cond}",
    ),
    (
        "for-fold",
        "A for fold: for x in xs, acc = init { body }",
        "for ${1:x} in ${2:xs}, ${3:acc} = ${4:init} {\n  ${5:// body}\n}",
    ),
    (
        "match-list",
        "A match on a list with empty and cons patterns",
        "match ${1:xs} {\n  []: ${2:// base case},\n  [h, ...t]: ${3:// recursive case}\n}",
    ),
    (
        "seq",
        "Sequential composition: a >>> b",
        "${1:first} >>> ${2:second}",
    ),
    (
        "prod",
        "Parallel product: a *** b",
        "${1:left} *** ${2:right}",
    ),
    ("fan", "Fan-out: a &&& b", "${1:left} &&& ${2:right}"),
    ("choice", "Choice: a ||| b", "${1:left} ||| ${2:right}"),
    (
        "nullish",
        "Nullish coalescing: expr ?? default",
        "${1:expr} ?? ${2:default}",
    ),
    (
        "opt-chain",
        "Optional chaining: expr?.field",
        "${1:expr}?.${2:field}",
    ),
    ("spread", "Spread: ...expr", "...${1:expr}"),
    (
        "record",
        "A record literal: { field: value }",
        "{\n  ${1:field}: ${2:value}\n}",
    ),
    ("tuple", "A tuple/positional: [items]", "[${1:items}]"),
    ("binding", "A binding: name = expr", "${1:name} = ${2:expr}"),
    (
        "binding-typed",
        "A typed binding: name: Type = expr",
        "${1:name}: ${2:Type} = ${3:expr}",
    ),
    (
        "doc",
        "A doc comment block",
        "/// ${1:name}: ${2:description}\n///\n/// ${3:details}",
    ),
];

/// Return all snippet completion items (ADR 0002, Step 2).
///
/// Each snippet is returned as a completion item with `kind = 15` (Snippet),
/// `insertTextFormat = 2` (Snippet), and the snippet body in VS Code snippet
/// syntax. These items are available in every completion context — the user
/// can type any trigger word anywhere and get the corresponding template.
pub fn snippet_completions() -> Vec<Value> {
    SNIPPETS
        .iter()
        .map(|&(label, detail, body)| {
            json!({
                "label": label,
                "kind": KIND_SNIPPET,
                "detail": detail,
                "insertText": body,
                "insertTextFormat": 2,
            })
        })
        .collect()
}

/// All known arrow combinators, offered when the cursor sits at a position
/// where an arrow expression is expected (after `=`, `<-`, or at the start of
/// a line inside a `do {}` or `rec {}` block). In v0.1 arrows are written with
/// `~>` and composed with `>>>`, `***`, `&&&`, `|||`; `rec` is the feedback
/// loop combinator.
const COMBINATORS: &[(&str, &str, &str)] = &[
    (
        ">>>",
        "arrow: a >>> b — sequential composition",
        "${1:a} >>> ${2:b}",
    ),
    (
        "***",
        "arrow: a *** b — parallel product",
        "${1:a} *** ${2:b}",
    ),
    ("&&&", "arrow: a &&& b — fan-out", "${1:a} &&& ${2:b}"),
    ("|||", "arrow: a ||| b — choice", "${1:a} ||| ${2:b}"),
    (
        "rec",
        "arrow: rec block — feedback loop",
        "rec {\n  ${1:// state}\n}",
    ),
    ("id", "arrow: identity", "id"),
];

/// Classify an identifier token by its syntactic role, returning
/// `(kind, detail)` for the completion item.
fn classify_identifier(lang: &Language, tokens: &[SpannedToken], index: usize) -> (i64, &'static str) {
    let is_kind = |i: usize, c: TC| -> bool {
        tokens.get(i).map(|t| lang.token_is(&t.kind, c)).unwrap_or(false)
    };

    // Property access: preceded by `.` → property or method
    if index > 0 && is_kind(index - 1, TC::Dot) {
        if is_kind(index + 1, TC::OpenParen) {
            return (KIND_FUNCTION, "method");
        }
        return (KIND_PROPERTY, "property");
    }

    // PascalCase → type
    let ident = tokens[index].text.as_str();
    if ident
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
        && ident.chars().count() > 1
        && !ident.contains('.')
    {
        return (KIND_CLASS, "type");
    }

    // `type.foo` → type reference (handled by lookahead for dot)
    if ident == "type" && is_kind(index + 1, TC::Dot) {
        return (KIND_CLASS, "type");
    }

    // Function call: next token is `(`
    if is_kind(index + 1, TC::OpenParen) {
        return (KIND_FUNCTION, "function");
    }

    // Dotted call: `ident . ident (`
    if is_kind(index + 1, TC::Dot)
        && tokens
            .get(index + 2)
            .map(|t| is_identifier_kind(lang, &t.kind))
            .unwrap_or(false)
        && is_kind(index + 3, TC::OpenParen)
    {
        return (KIND_FUNCTION, "function");
    }

    // `key:` inside parens → parameter; inside braces → property
    if is_kind(index + 1, TC::Colon) {
        return match enclosing_open_delim(lang, tokens, index) {
            Some(TC::OpenParen) => (KIND_PARAMETER, "parameter"),
            Some(TC::OpenBrace) => (KIND_PROPERTY, "property"),
            _ => (KIND_VARIABLE, "binding"),
        };
    }

    // Binding declaration: at line start, followed by `:` or `::`
    if is_kind(index + 1, TC::Colon) || is_kind(index + 1, TC::Bind) {
        return (KIND_VARIABLE, "binding");
    }

    (KIND_VARIABLE, "variable")
}

/// Find the nearest unmatched opening delimiter enclosing `index`.
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

/// Detect whether the cursor is inside an arrow-graph expression (i.e. the
/// file looks like an arrow file). If so, arrow combinator snippets are
/// offered alongside keywords.
fn looks_like_arrow_file(source: &str) -> bool {
    source.contains("~>")
        || source.contains("rec")
        || source.contains(">>>")
        || source.contains("|||")
        || source.contains("***")
        || source.contains("&&&")
}

pub fn completions(source: &str) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();

    // 1. Keyword completions (with snippets for structural forms).
    for entry in KEYWORDS {
        if let Some(body) = entry.snippet {
            items.push(json!({
                "label": entry.text,
                "kind": KIND_SNIPPET,
                "detail": entry.detail,
                "insertText": body,
                "insertTextFormat": 2,
            }));
        } else {
            let kind = if entry.text == "true" || entry.text == "false" {
                KIND_CONSTANT
            } else if entry.text == "null" {
                KIND_CONSTANT
            } else {
                KIND_KEYWORD
            };
            items.push(json!({
                "label": entry.text,
                "kind": kind,
                "detail": entry.detail,
            }));
        }
    }

    // 2. Arrow combinator snippets (only in arrow-graph context).
    if looks_like_arrow_file(source) {
        for &(text, detail, body) in ARROW_SNIPPETS {
            items.push(json!({
                "label": text,
                "kind": KIND_SNIPPET,
                "detail": detail,
                "insertText": body,
                "insertTextFormat": 2,
            }));
        }
    }

    // 3. Identifier completions, classified by role.
    if let Ok(kit) = kit_for_file(None) {
        let lang = kit.lang();
        if let Ok(tokens) = kit.lex_with_spans(source) {
            // Collect unique names with their best (most specific) role.
            // Prefer function > type > parameter/property > binding > variable.
            let mut seen: BTreeMap<String, (i64, &'static str)> = BTreeMap::new();
            for (i, token) in tokens.as_slice().iter().enumerate() {
                if !is_identifier_kind(lang, &token.kind) {
                    continue;
                }
                let role = classify_identifier(lang, &tokens, i);
                let entry = seen.entry(token.text.clone()).or_insert(role);
                // Upgrade: function > class > parameter/property > binding > variable
                if role.0 == KIND_FUNCTION
                    || (entry.0 != KIND_FUNCTION && role.0 == KIND_CLASS)
                    || (entry.0 != KIND_FUNCTION && entry.0 != KIND_CLASS && role.1 == "parameter")
                    || (entry.0 != KIND_FUNCTION && entry.0 != KIND_CLASS && role.1 == "property")
                {
                    *entry = role;
                }
            }
            for (name, (kind, detail)) in seen {
                items.push(json!({ "label": name, "kind": kind, "detail": detail }));
            }
        }
    }

    // 4. Snippet templates (ADR 0002, Step 2) — available in every context.
    //    Merged with the existing completions and deduplicated by label so that
    //    a keyword or in-scope identifier with the same name takes precedence
    //    (the keyword-triggered snippets are already emitted in step 1).
    let existing_labels: std::collections::HashSet<String> = items
        .iter()
        .filter_map(|i| i["label"].as_str().map(|s| s.to_string()))
        .collect();
    for item in snippet_completions() {
        let label = item["label"].as_str().unwrap_or("");
        if !existing_labels.contains(label) {
            items.push(item);
        }
    }

    items
}

// ---------------------------------------------------------------------------
// Context-aware completion
// ---------------------------------------------------------------------------

/// The syntactic context at the cursor position, used to drive context-aware
/// completion suggestions.
enum Context {
    /// No specific context recognised — fall back to the full completion set.
    None,
    /// Cursor right after the `rec` keyword — suggest the state block `{ ... }`.
    AfterRec,
    /// Cursor right after the `match` keyword — suggest `expr { ... }`.
    AfterMatch,
    /// Cursor right after the `do` keyword — suggest the `do { ... } while` body.
    AfterDo,
    /// Cursor right after the `return` keyword — suggest `expr`.
    AfterReturn,
    /// Cursor right after `state.` (or after the `{` of a rec state block) —
    /// suggest the field names declared in the rec state.
    StateField,
    /// Cursor at a position where an arrow expression is expected — suggest
    /// the known combinators.
    ArrowExpr,
    /// Cursor right after a pattern colon inside a `match` block — suggest
    /// the empty/non-empty list patterns.
    MatchPattern,
}

/// Index of the last token whose span ends at or before `offset` (i.e. the
/// token immediately preceding the cursor). Returns `None` when the cursor
/// sits before any token.
fn preceding_index(tokens: &[SpannedToken], offset: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .rev()
        .find(|(_, t)| t.span.end <= offset)
        .map(|(i, _)| i)
}

/// Index of the unmatched `{` that encloses the cursor position, scanning only
/// tokens that begin before `offset`. Returns `None` when the cursor is not
/// inside any brace block.
fn enclosing_lbrace_index_before(
    lang: &Language,
    tokens: &[SpannedToken],
    offset: usize,
) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = tokens.len();
    while i > 0 {
        i -= 1;
        if tokens[i].span.start >= offset {
            continue;
        }
        match lang.token_category(tokens[i].kind.as_str()) {
            Some(TC::CloseBrace) => depth += 1,
            Some(TC::OpenBrace) if depth == 0 => return Some(i),
            Some(TC::OpenBrace) => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Whether a given keyword text appears immediately before a `{` brace (with
/// no intervening nested braces). Used to recognise `match … {` and `do {`
/// blocks by inspecting the brace's opening token.
fn keyword_before_brace(
    lang: &Language,
    tokens: &[SpannedToken],
    brace_idx: usize,
    kw: &str,
) -> bool {
    let mut depth: i32 = 0;
    let mut j = brace_idx;
    while j > 0 {
        j -= 1;
        match lang.token_category(&tokens[j].kind) {
            Some(TC::CloseBrace) => depth += 1,
            Some(TC::OpenBrace) => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            _ if depth == 0 && is_keyword_kind(lang, &tokens[j].kind) && tokens[j].text == kw => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Whether the cursor sits inside a `do { … }` or `rec { … }` block.
fn inside_do_block(lang: &Language, tokens: &[SpannedToken], offset: usize) -> bool {
    enclosing_lbrace_index_before(lang, tokens, offset)
        .map(|lb| {
            keyword_before_brace(lang, tokens, lb, "do")
                || keyword_before_brace(lang, tokens, lb, "rec")
        })
        .unwrap_or(false)
}

/// Whether the colon at `colon_idx` is a pattern colon inside a `match` block
/// (i.e. the token preceding the colon is `]` and the enclosing brace belongs
/// to a `match` expression).
fn is_match_pattern_colon(lang: &Language, tokens: &[SpannedToken], colon_idx: usize) -> bool {
    if colon_idx == 0 {
        return false;
    }
    if !lang.token_is(&tokens[colon_idx - 1].kind, TC::CloseBracket) {
        return false;
    }
    let offset = tokens[colon_idx].span.end;
    enclosing_lbrace_index_before(lang, tokens, offset)
        .map(|lb| keyword_before_brace(lang, tokens, lb, "match"))
        .unwrap_or(false)
}

/// Extract the field names declared in a rec state block
/// (`rec { name = value, … }`). Scans every `rec` keyword in the source and
/// collects the identifiers that are immediately followed by `=` inside the
/// state block's braces. (v0.1 uses `=` for bindings, not `:`.)
fn rec_state_fields(lang: &Language, tokens: &[SpannedToken]) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let is_rec = is_keyword_kind(lang, &tokens[i].kind) && tokens[i].text == "rec";
        let next_is_brace = tokens
            .get(i + 1)
            .map(|t| lang.token_is(&t.kind, TC::OpenBrace))
            .unwrap_or(false);
        if is_rec && next_is_brace {
            let mut j = i + 2;
            let mut depth: i32 = 1;
            while j < tokens.len() && depth > 0 {
                match lang.token_category(&tokens[j].kind) {
                    Some(TC::OpenBrace) => depth += 1,
                    Some(TC::CloseBrace) => depth -= 1,
                    _ => {}
                }
                if depth > 0 && is_identifier_kind(lang, &tokens[j].kind) {
                    // Rec state fields are `name : init` entries (`token.colon`);
                    // `bind` (`::`) is also accepted for resilience.
                    let followed_by_field_sep = tokens
                        .get(j + 1)
                        .map(|t| {
                            lang.token_is(&t.kind, TC::Colon)
                                || lang.token_is(&t.kind, TC::Bind)
                        })
                        .unwrap_or(false);
                    if followed_by_field_sep {
                        let name = tokens[j].text.clone();
                        if !fields.contains(&name) {
                            fields.push(name);
                        }
                    }
                }
                j += 1;
            }
        }
        i += 1;
    }
    fields
}

/// Classify the syntactic context at `offset` within the token stream.
fn detect_context(lang: &Language, tokens: &[SpannedToken], offset: usize, source: &str) -> Context {
    let idx = match preceding_index(tokens, offset) {
        Some(i) => i,
        None => {
            // Cursor before any token: only suggest combinators if we are
            // inside a `do {}` block (start of a statement).
            return if inside_do_block(lang, tokens, offset) {
                Context::ArrowExpr
            } else {
                Context::None
            };
        }
    };
    let kind = tokens[idx].kind.as_str();
    let text = tokens[idx].text.as_str();

    // 1. Right after a structural keyword.
    if is_keyword_kind(lang, kind) {
        match text {
            "rec" => return Context::AfterRec,
            "match" => return Context::AfterMatch,
            "do" => return Context::AfterDo,
            "return" => return Context::AfterReturn,
            _ => {}
        }
    }

    // 2. Field access `state.` → suggest in-scope rec state fields.
    if lang.token_is(kind, TC::Dot)
        && idx > 0
        && is_identifier_kind(lang, &tokens[idx - 1].kind)
        && tokens[idx - 1].text == "state"
    {
        return Context::StateField;
    }

    // 2b. Right after the `{` opening a rec state block → field names.
    if lang.token_is(kind, TC::OpenBrace)
        && idx > 0
        && is_keyword_kind(lang, &tokens[idx - 1].kind)
        && tokens[idx - 1].text == "rec"
    {
        return Context::StateField;
    }

    // 3. After a pattern colon inside a `match` block → list patterns.
    if lang.token_is(kind, TC::Colon) && is_match_pattern_colon(lang, tokens, idx) {
        return Context::MatchPattern;
    }

    // 4. Arrow expression position: after `<-` (always arrow-specific) or
    //    after `::` when in a binding context.
    // `<-` is lexed as two tokens: `<` followed by `-` (lexeme check — both
    // are `operator` tokens; the glyph pair is what matters here).
    if text == "-" && idx > 0 && tokens[idx - 1].text == "<" {
        return Context::ArrowExpr;
    }
    if lang.token_is(kind, TC::Bind)
        && (looks_like_arrow_file(source) || inside_do_block(lang, tokens, offset)) {
        return Context::ArrowExpr;
    }

    // 4b. Start of a new line inside a `do {}` or `rec {}` block (after `;`,
    //     `}`, or the block's opening `{`) → arrow expression position.
    let cat = lang.token_category(kind);
    if (cat == Some(TC::Semicolon) || cat == Some(TC::CloseBrace) || cat == Some(TC::OpenBrace))
        && inside_do_block(lang, tokens, offset)
        && !(cat == Some(TC::OpenBrace)
            && idx > 0
            && is_keyword_kind(lang, &tokens[idx - 1].kind)
            && tokens[idx - 1].text == "rec")
    {
        return Context::ArrowExpr;
    }

    Context::None
}

/// Context-aware completion entry point.
///
/// Returns a JSON array of completion items tailored to the cursor position
/// `offset` (a byte offset into `source`). When no specific context is
/// recognised, the full keyword/identifier set from [`completions`] is
/// returned so the function remains useful everywhere.
pub fn completion(source: &str, offset: usize) -> Value {
    let mut items = completions(source);

    let Some(kit) = kit_for_file(None).ok() else {
        return json!(items);
    };
    let lang = kit.lang();
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return json!(items),
    };

    match detect_context(lang, &tokens, offset, source) {
        Context::None => {}
        Context::AfterRec => items.push(json!({
            "label": "{",
            "kind": KIND_SNIPPET,
            "detail": "rec state block",
            "insertText": "{\n  ${1:remaining} = ${2:input},\n  ${3:acc} = []\n}",
            "insertTextFormat": 2,
        })),
        Context::AfterMatch => items.push(json!({
            "label": "expr {",
            "kind": KIND_SNIPPET,
            "detail": "match scrutinee and arms",
            "insertText": "${1:expr} {\n  []: ${2:base},\n  [h, ...t]: ${3:step}\n}",
            "insertTextFormat": 2,
        })),
        Context::AfterDo => items.push(json!({
            "label": "{",
            "kind": KIND_SNIPPET,
            "detail": "do-while body",
            "insertText": "{\n  ${1:body}\n} while ${2:cond}",
            "insertTextFormat": 2,
        })),
        Context::AfterReturn => items.push(json!({
            "label": "expr",
            "kind": KIND_SNIPPET,
            "detail": "return value expression",
            "insertText": "${1:expr}",
            "insertTextFormat": 2,
        })),
        Context::StateField => {
            for field in rec_state_fields(lang, &tokens) {
                items.push(json!({
                    "label": field,
                    "kind": KIND_PROPERTY,
                    "detail": "rec state field",
                }));
            }
        }
        Context::ArrowExpr => {
            for &(text, detail, body) in COMBINATORS {
                items.push(json!({
                    "label": text,
                    "kind": KIND_SNIPPET,
                    "detail": detail,
                    "insertText": body,
                    "insertTextFormat": 2,
                }));
            }
        }
        Context::MatchPattern => {
            items.push(json!({
                "label": "[]",
                "kind": KIND_SNIPPET,
                "detail": "match pattern: empty list",
                "insertText": "[]: ${1:body}",
                "insertTextFormat": 2,
            }));
            items.push(json!({
                "label": "[h, ...t]",
                "kind": KIND_SNIPPET,
                "detail": "match pattern: non-empty list",
                "insertText": "[h, ...t]: ${1:body}",
                "insertTextFormat": 2,
            }));
        }
    }

    json!(items)
}
