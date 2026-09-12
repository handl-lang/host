//! LSP completion plugin (ADR 0002 / completion decoupling).

use std::collections::{BTreeMap, HashSet};
use std::fs::{read_dir, read_to_string};
use std::path::{Path, PathBuf};

use handl_host_tooling::lsp::{
    CompletionItem, CompletionPlugin, CompletionRequest, CompletionResponse,
};
use handl_kit_canonical::{Kit, SpannedToken, TokenClasses};
use handl_platform::PlatformError;

/// LSP CompletionItemKind values.
const KIND_KEYWORD: i64 = 14;
const KIND_SNIPPET: i64 = 15;
const KIND_VARIABLE: i64 = 6;
const KIND_FUNCTION: i64 = 3;
const KIND_CLASS: i64 = 7;
const KIND_PROPERTY: i64 = 8;
const KIND_PARAMETER: i64 = 6;
const KIND_CONSTANT: i64 = 21;
const KIND_FILE: i64 = 17;

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
pub fn snippet_completions() -> Vec<CompletionItem> {
    SNIPPETS
        .iter()
        .map(|&(label, detail, body)| {
            let mut item = CompletionItem::new(label);
            item.kind = Some(KIND_SNIPPET);
            item.detail = Some(detail.to_string());
            item.insert_text = Some(body.to_string());
            item.insert_text_format = Some(2);
            item
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
fn classify_identifier(
    classes: &TokenClasses,
    tokens: &[SpannedToken],
    index: usize,
) -> (i64, &'static str) {
    let is_label = |i: usize, l: &str| -> bool {
        tokens
            .get(i)
            .map(|t| classes.is_label(&t.kind, l))
            .unwrap_or(false)
    };

    // Property access: preceded by `.` → property or method
    if index > 0 && is_label(index - 1, ".") {
        if is_label(index + 1, "(") {
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
    if ident == "type" && is_label(index + 1, ".") {
        return (KIND_CLASS, "type");
    }

    // Function call: next token is `(`
    if is_label(index + 1, "(") {
        return (KIND_FUNCTION, "function");
    }

    // Dotted call: `ident . ident (`
    if is_label(index + 1, ".")
        && tokens
            .get(index + 2)
            .map(|t| classes.is_identifier(&t.kind))
            .unwrap_or(false)
        && is_label(index + 3, "(")
    {
        return (KIND_FUNCTION, "function");
    }

    // `key:` inside parens → parameter; inside braces → property
    if is_label(index + 1, ":") {
        return match enclosing_open_delim(classes, tokens, index) {
            Some(open) if classes.is_label(open, "(") => (KIND_PARAMETER, "parameter"),
            Some(open) if classes.is_label(open, "{") => (KIND_PROPERTY, "property"),
            _ => (KIND_VARIABLE, "binding"),
        };
    }

    // Binding declaration: at line start, followed by `:` or `::`
    if is_label(index + 1, ":") || is_label(index + 1, "::") {
        return (KIND_VARIABLE, "binding");
    }

    (KIND_VARIABLE, "variable")
}

/// Find the kind of the nearest unmatched opening delimiter enclosing
/// `index`, resolved via the language's `open-bracket`/`close-bracket`
/// categories and `pair` links.
fn enclosing_open_delim<'a>(
    classes: &TokenClasses,
    tokens: &'a [SpannedToken],
    index: usize,
) -> Option<&'a str> {
    // open-kind → count of seen closers expecting it.
    let mut depth: std::collections::HashMap<&str, i32> = std::collections::HashMap::new();
    let mut i = index;
    while i > 0 {
        i -= 1;
        let kind = tokens[i].kind.as_str();
        if classes.is_close_bracket(kind) {
            if let Some(open) = classes.pair_of(kind) {
                *depth.entry(open).or_insert(0) += 1;
            }
        } else if classes.is_open_bracket(kind) {
            match depth.get_mut(kind) {
                Some(d) if *d > 0 => *d -= 1,
                _ => return Some(kind),
            }
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

pub fn completions(source: &str) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = Vec::new();

    // 1. Keyword completions (with snippets for structural forms).
    for entry in KEYWORDS {
        let mut item = CompletionItem::new(entry.text);
        item.detail = Some(entry.detail.to_string());
        if let Some(body) = entry.snippet {
            item.kind = Some(KIND_SNIPPET);
            item.insert_text = Some(body.to_string());
            item.insert_text_format = Some(2);
        } else {
            item.kind = Some(if entry.text == "true" || entry.text == "false" || entry.text == "null" {
                KIND_CONSTANT
            } else {
                KIND_KEYWORD
            });
        }
        items.push(item);
    }

    // 2. Arrow combinator snippets (only in arrow-graph context).
    if looks_like_arrow_file(source) {
        for &(text, detail, body) in ARROW_SNIPPETS {
            let mut item = CompletionItem::new(text);
            item.kind = Some(KIND_SNIPPET);
            item.detail = Some(detail.to_string());
            item.insert_text = Some(body.to_string());
            item.insert_text_format = Some(2);
            items.push(item);
        }
    }

    // 3. Identifier completions, classified by role.
    if let Ok(kit) = Kit::new() {
        let classes = TokenClasses::new(kit.language());
        if let Ok(tokens) = kit.lex_with_spans(source) {
            // Collect unique names with their best (most specific) role.
            // Prefer function > type > parameter/property > binding > variable.
            let mut seen: BTreeMap<String, (i64, &'static str)> = BTreeMap::new();
            for (i, token) in tokens.as_slice().iter().enumerate() {
                if !classes.is_identifier(&token.kind) {
                    continue;
                }
                let role = classify_identifier(&classes, &tokens, i);
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
                let mut item = CompletionItem::new(name);
                item.kind = Some(kind);
                item.detail = Some(detail.to_string());
                items.push(item);
            }
        }
    }

    // 4. Snippet templates (ADR 0002, Step 2) — available in every context.
    //    Merged with the existing completions and deduplicated by label so that
    //    a keyword or in-scope identifier with the same name takes precedence
    //    (the keyword-triggered snippets are already emitted in step 1).
    let existing_labels: HashSet<String> = items.iter().map(|i| i.label.clone()).collect();
    for item in snippet_completions() {
        if !existing_labels.contains(&item.label) {
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
    classes: &TokenClasses,
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
        let kind = tokens[i].kind.as_str();
        if classes.is_label(kind, "}") {
            depth += 1;
        } else if classes.is_label(kind, "{") {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
    }
    None
}

/// Whether a given keyword text appears immediately before a `{` brace (with
/// no intervening nested braces). Used to recognise `match … {` and `do {`
/// blocks by inspecting the brace's opening token.
fn keyword_before_brace(
    classes: &TokenClasses,
    tokens: &[SpannedToken],
    brace_idx: usize,
    kw: &str,
) -> bool {
    let mut depth: i32 = 0;
    let mut j = brace_idx;
    while j > 0 {
        j -= 1;
        let kind = tokens[j].kind.as_str();
        if classes.is_label(kind, "}") {
            depth += 1;
        } else if classes.is_label(kind, "{") {
            if depth == 0 {
                return false;
            }
            depth -= 1;
        } else if depth == 0 && classes.is_keyword(&tokens[j].kind) && tokens[j].text == kw {
            return true;
        }
    }
    false
}

/// Whether the cursor sits inside a `do { … }` or `rec { … }` block.
fn inside_do_block(classes: &TokenClasses, tokens: &[SpannedToken], offset: usize) -> bool {
    enclosing_lbrace_index_before(classes, tokens, offset)
        .map(|lb| {
            keyword_before_brace(classes, tokens, lb, "do")
                || keyword_before_brace(classes, tokens, lb, "rec")
        })
        .unwrap_or(false)
}

/// Whether the colon at `colon_idx` is a pattern colon inside a `match` block
/// (i.e. the token preceding the colon is `]` and the enclosing brace belongs
/// to a `match` expression).
fn is_match_pattern_colon(
    classes: &TokenClasses,
    tokens: &[SpannedToken],
    colon_idx: usize,
) -> bool {
    if colon_idx == 0 {
        return false;
    }
    if !classes.is_label(&tokens[colon_idx - 1].kind, "]") {
        return false;
    }
    let offset = tokens[colon_idx].span.end;
    enclosing_lbrace_index_before(classes, tokens, offset)
        .map(|lb| keyword_before_brace(classes, tokens, lb, "match"))
        .unwrap_or(false)
}

/// Extract the field names declared in a rec state block
/// (`rec { name = value, … }`). Scans every `rec` keyword in the source and
/// collects the identifiers that are immediately followed by `=` inside the
/// state block's braces. (v0.1 uses `=` for bindings, not `:`.)
fn rec_state_fields(classes: &TokenClasses, tokens: &[SpannedToken]) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let is_rec = classes.is_keyword(&tokens[i].kind) && tokens[i].text == "rec";
        let next_is_brace = tokens
            .get(i + 1)
            .map(|t| classes.is_label(&t.kind, "{"))
            .unwrap_or(false);
        if is_rec && next_is_brace {
            let mut j = i + 2;
            let mut depth: i32 = 1;
            while j < tokens.len() && depth > 0 {
                let k = tokens[j].kind.as_str();
                if classes.is_label(k, "{") {
                    depth += 1;
                } else if classes.is_label(k, "}") {
                    depth -= 1;
                }
                if depth > 0 && classes.is_identifier(&tokens[j].kind) {
                    // Rec state fields are `name : init` entries (`:`);
                    // `::` is also accepted for resilience.
                    let followed_by_field_sep = tokens
                        .get(j + 1)
                        .map(|t| classes.is_label(&t.kind, ":") || classes.is_label(&t.kind, "::"))
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
fn detect_context(
    classes: &TokenClasses,
    tokens: &[SpannedToken],
    offset: usize,
    source: &str,
) -> Context {
    let idx = match preceding_index(tokens, offset) {
        Some(i) => i,
        None => {
            // Cursor before any token: only suggest combinators if we are
            // inside a `do {}` block (start of a statement).
            return if inside_do_block(classes, tokens, offset) {
                Context::ArrowExpr
            } else {
                Context::None
            };
        }
    };
    let kind = tokens[idx].kind.as_str();
    let text = tokens[idx].text.as_str();

    // 1. Right after a structural keyword.
    if classes.is_keyword(kind) {
        match text {
            "rec" => return Context::AfterRec,
            "match" => return Context::AfterMatch,
            "do" => return Context::AfterDo,
            "return" => return Context::AfterReturn,
            _ => {}
        }
    }

    // 2. Field access `state.` → suggest in-scope rec state fields.
    if classes.is_label(kind, ".")
        && idx > 0
        && classes.is_identifier(&tokens[idx - 1].kind)
        && tokens[idx - 1].text == "state"
    {
        return Context::StateField;
    }

    // 2b. Right after the `{` opening a rec state block → field names.
    if classes.is_label(kind, "{")
        && idx > 0
        && classes.is_keyword(&tokens[idx - 1].kind)
        && tokens[idx - 1].text == "rec"
    {
        return Context::StateField;
    }

    // 3. After a pattern colon inside a `match` block → list patterns.
    if classes.is_label(kind, ":") && is_match_pattern_colon(classes, tokens, idx) {
        return Context::MatchPattern;
    }

    // 4. Arrow expression position: after `<-` (always arrow-specific) or
    //    after `::` when in a binding context.
    // `<-` is lexed as two tokens: `<` followed by `-`.
    if classes.is_label(kind, "-") && idx > 0 && classes.is_label(&tokens[idx - 1].kind, "<") {
        return Context::ArrowExpr;
    }
    if classes.is_label(kind, "::")
        && (looks_like_arrow_file(source) || inside_do_block(classes, tokens, offset))
    {
        return Context::ArrowExpr;
    }

    // 4b. Start of a new line inside a `do {}` or `rec {}` block (after `;`,
    //     `}`, or the block's opening `{`) → arrow expression position.
    if (classes.is_label(kind, ";") || classes.is_label(kind, "}") || classes.is_label(kind, "{"))
        && inside_do_block(classes, tokens, offset)
        && !(classes.is_label(kind, "{")
            && idx > 0
            && classes.is_keyword(&tokens[idx - 1].kind)
            && tokens[idx - 1].text == "rec")
    {
        return Context::ArrowExpr;
    }

    Context::None
}

/// Context-aware completion entry point.
///
/// Returns a list of completion items tailored to the cursor position
/// `offset` (a byte offset into `source`). When no specific context is
/// recognised, the full keyword/identifier set from [`completions`] is
/// returned so the function remains useful everywhere.
pub fn completion(source: &str, offset: usize) -> Vec<CompletionItem> {
    let mut items = completions(source);

    if let Ok((tokens, kit)) =
        Kit::new().and_then(|kit| kit.lex_with_spans(source).map(|t| (t, kit)))
    {
        let classes = TokenClasses::new(kit.language());
        match detect_context(&classes, &tokens, offset, source) {
            Context::None => {}
            Context::AfterRec => {
                let mut item = CompletionItem::new("{");
                item.kind = Some(KIND_SNIPPET);
                item.detail = Some("rec state block".to_string());
                item.insert_text =
                    Some("{\n  ${1:remaining} = ${2:input},\n  ${3:acc} = []\n}".to_string());
                item.insert_text_format = Some(2);
                items.push(item);
            }
            Context::AfterMatch => {
                let mut item = CompletionItem::new("expr {");
                item.kind = Some(KIND_SNIPPET);
                item.detail = Some("match scrutinee and arms".to_string());
                item.insert_text = Some(
                    "${1:expr} {\n  []: ${2:base},\n  [h, ...t]: ${3:step}\n}".to_string(),
                );
                item.insert_text_format = Some(2);
                items.push(item);
            }
            Context::AfterDo => {
                let mut item = CompletionItem::new("{");
                item.kind = Some(KIND_SNIPPET);
                item.detail = Some("do-while body".to_string());
                item.insert_text =
                    Some("{\n  ${1:body}\n} while ${2:cond}".to_string());
                item.insert_text_format = Some(2);
                items.push(item);
            }
            Context::AfterReturn => {
                let mut item = CompletionItem::new("expr");
                item.kind = Some(KIND_SNIPPET);
                item.detail = Some("return value expression".to_string());
                item.insert_text = Some("${1:expr}".to_string());
                item.insert_text_format = Some(2);
                items.push(item);
            }
            Context::StateField => {
                for field in rec_state_fields(&classes, &tokens) {
                    let mut item = CompletionItem::new(field);
                    item.kind = Some(KIND_PROPERTY);
                    item.detail = Some("rec state field".to_string());
                    items.push(item);
                }
            }
            Context::ArrowExpr => {
                for &(text, detail, body) in COMBINATORS {
                    let mut item = CompletionItem::new(text);
                    item.kind = Some(KIND_SNIPPET);
                    item.detail = Some(detail.to_string());
                    item.insert_text = Some(body.to_string());
                    item.insert_text_format = Some(2);
                    items.push(item);
                }
            }
            Context::MatchPattern => {
                let mut empty = CompletionItem::new("[]");
                empty.kind = Some(KIND_SNIPPET);
                empty.detail = Some("match pattern: empty list".to_string());
                empty.insert_text = Some("[]: ${1:body}".to_string());
                empty.insert_text_format = Some(2);
                items.push(empty);

                let mut nonempty = CompletionItem::new("[h, ...t]");
                nonempty.kind = Some(KIND_SNIPPET);
                nonempty.detail = Some("match pattern: non-empty list".to_string());
                nonempty.insert_text = Some("[h, ...t]: ${1:body}".to_string());
                nonempty.insert_text_format = Some(2);
                items.push(nonempty);
            }
        }
    }

    items
}

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

fn token_at(tokens: &[SpannedToken], pos: usize) -> Option<usize> {
    tokens
        .iter()
        .position(|t| t.span.start <= pos && pos <= t.span.end)
}

fn import_path_completions(
    tokens: &[SpannedToken],
    source: &str,
    pos: usize,
    path: &str,
    classes: &TokenClasses,
) -> Option<Vec<CompletionItem>> {
    let idx = token_at(tokens, pos)?;
    let tok = &tokens[idx];
    if !classes.is_string(&tok.kind) {
        return None;
    }
    let prev = tokens.get(idx.checked_sub(1)?)?;
    if !classes.is_keyword(&prev.kind) || (prev.text != "import" && prev.text != "from") {
        return None;
    }
    let base = base_dir(path)?;
    let start = tok.span.start + 1;
    let end = pos.min(tok.span.end);
    let mut prefix = &source[start..end];
    if prefix.ends_with('"') {
        prefix = &prefix[..prefix.len() - 1];
    }
    // Strip a trailing slash so that `../dir/|` completes inside `dir`,
    // not inside a non-existent `dir/`.
    let prefix = prefix.strip_suffix('/').unwrap_or(prefix);
    let (dir_part, stem) = if let Some(slash) = prefix.rfind('/') {
        (&prefix[..slash], &prefix[slash + 1..])
    } else {
        ("", prefix)
    };
    let mut search_dir = if dir_part.is_empty() {
        base
    } else {
        base.join(dir_part)
    };
    search_dir = search_dir.canonicalize().ok().unwrap_or(search_dir);
    let current_name = Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let mut items = Vec::new();
    for entry in read_dir(&search_dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(stem) {
            continue;
        }
        if dir_part.is_empty() && name == current_name {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let full = if is_dir {
            if dir_part.is_empty() {
                format!("{name}/")
            } else {
                format!("{dir_part}/{name}/")
            }
        } else if dir_part.is_empty() {
            name.clone()
        } else {
            format!("{dir_part}/{name}")
        };
        let label = if is_dir {
            format!("{name}/")
        } else {
            name.clone()
        };
        let mut item = CompletionItem::new(label);
        item.kind = Some(KIND_FILE);
        item.detail = Some(if is_dir { "directory".into() } else { "file".into() });
        item.insert_text = Some(full);
        items.push(item);
        // Also suggest `dir/dir.hnl` as a file when a directory carries the
        // canonical module-file layout.
        if is_dir {
            let child = entry.path().join(format!("{name}.hnl"));
            if child.is_file() {
                let child_full = if dir_part.is_empty() {
                    format!("{name}.hnl")
                } else {
                    format!("{dir_part}/{name}.hnl")
                };
                let mut child_item = CompletionItem::new(format!("{name}.hnl"));
                child_item.kind = Some(KIND_FILE);
                child_item.detail = Some("module file".into());
                child_item.insert_text = Some(child_full);
                items.push(child_item);
            }
        }
    }
    if items.is_empty() {
        return None;
    }
    items.sort_by(|a, b| a.label.cmp(&b.label));
    Some(items)
}

fn resolve_import_target(base: &Path, spec: &str) -> Option<PathBuf> {
    let p = base.join(spec);
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

fn exported_names(source: &str, kit: &Kit) -> Vec<String> {
    let Ok(tokens) = kit.lex_with_spans(source) else {
        return Vec::new();
    };
    let classes = TokenClasses::new(kit.language());
    let mut names: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut depth: i32 = 0;
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
        if classes.is_keyword(kind) && (tokens[i].text == "axiom" || tokens[i].text == "law") {
            let mut j = i + 1;
            let mut parts = Vec::new();
            let mut expect_ident = true;
            while j < tokens.len() {
                if expect_ident {
                    if classes.is_identifier(&tokens[j].kind) {
                        parts.push(tokens[j].text.clone());
                        expect_ident = false;
                        j += 1;
                    } else {
                        break;
                    }
                } else if classes.is_label(&tokens[j].kind, ".") {
                    expect_ident = true;
                    j += 1;
                } else {
                    break;
                }
            }
            if !parts.is_empty() {
                let full = parts.join(".");
                if seen.insert(full.clone()) {
                    names.push(full);
                }
                if parts.len() > 1 {
                    if let Some(last) = parts.last() {
                        if seen.insert(last.clone()) {
                            names.push(last.clone());
                        }
                    }
                }
            }
            i = j;
            continue;
        }
        if classes.is_identifier(kind) {
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

fn import_export_completions(
    tokens: &[SpannedToken],
    pos: usize,
    path: &str,
    classes: &TokenClasses,
    kit: &Kit,
) -> Option<Vec<CompletionItem>> {
    let idx = preceding_index(tokens, pos)?;
    let import_idx = if classes.is_keyword(&tokens[idx].kind) && tokens[idx].text == "import" {
        Some(idx)
    } else if classes.is_identifier(&tokens[idx].kind)
        && idx > 0
        && classes.is_keyword(&tokens[idx - 1].kind)
        && tokens[idx - 1].text == "import"
    {
        Some(idx - 1)
    } else {
        None
    }?;
    let spec = tokens.get(import_idx.checked_sub(1)?)?;
    if !classes.is_string(&spec.kind) {
        return None;
    }
    let base = base_dir(path)?;
    let raw = spec
        .text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(&spec.text);
    let target = resolve_import_target(&base, raw)?;
    let src = read_to_string(&target).ok()?;
    let names = exported_names(&src, kit);
    if names.is_empty() {
        return None;
    }
    let mut items = Vec::new();
    for name in names {
        let mut item = CompletionItem::new(name.clone());
        item.kind = Some(KIND_VARIABLE);
        item.detail = Some("exported name".into());
        item.insert_text = Some(name);
        items.push(item);
    }
    Some(items)
}

fn import_completions(source: &str, pos: usize, path: &str) -> Option<Vec<CompletionItem>> {
    let kit = Kit::new().ok()?;
    let tokens = kit.lex_with_spans(source).ok()?;
    let classes = TokenClasses::new(kit.language());
    if let Some(items) = import_path_completions(&tokens, source, pos, path, &classes) {
        return Some(items);
    }
    if let Some(items) = import_export_completions(&tokens, pos, path, &classes, &kit) {
        return Some(items);
    }
    None
}

/// A minimal completion plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicCompletion;

impl BasicCompletion {
    pub fn new() -> Self {
        Self
    }
}

impl CompletionPlugin for BasicCompletion {
    fn run(&self, request: &CompletionRequest) -> Result<CompletionResponse, PlatformError> {
        if let Some(items) = import_completions(&request.source, request.pos, &request.path) {
            return Ok(CompletionResponse(items));
        }
        Ok(CompletionResponse(completion(&request.source, request.pos)))
    }
}
