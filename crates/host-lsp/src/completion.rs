use std::collections::BTreeMap;

use serde_json::{Value, json};

use handl_kit_canonical::{Kit, SpannedToken, is_identifier_kind, is_keyword_kind};

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
    KeywordEntry { text: "match", detail: "keyword: match expression", snippet: Some("match ${1:expr} {\n  ${2:pat}: ${3:body}\n}") },
    KeywordEntry { text: "if", detail: "keyword: conditional", snippet: Some("if ${1:cond} then ${2:a} else ${3:b}") },
    KeywordEntry { text: "then", detail: "keyword: then-branch of if", snippet: None },
    KeywordEntry { text: "else", detail: "keyword: else-branch of if", snippet: None },
    KeywordEntry { text: "loop", detail: "keyword: arrow loop combinator", snippet: Some("loop { ${1:state} }(\n  proc (${2:state}) -> match ${2:state}.${3:field} {\n    []: { out: ${4:acc} },\n    [h, ...t]: { state: ${5:next} }\n  }\n)") },
    KeywordEntry { text: "proc", detail: "keyword: procedure (arrow body)", snippet: Some("proc (${1:state}) -> ${2:body}") },
    KeywordEntry { text: "where", detail: "keyword: where-binding", snippet: Some("where\n  ${1:name} = ${2:expr}") },
    KeywordEntry { text: "axiom", detail: "keyword: axiom declaration", snippet: Some("axiom ${1:name} : ${2:signature}") },
    KeywordEntry { text: "in", detail: "keyword: let-in expression", snippet: None },
    KeywordEntry { text: "handle", detail: "keyword: handle effect", snippet: Some("handle ${1:expr} { ${2:cases} }") },
    KeywordEntry { text: "as", detail: "keyword: type ascription", snippet: None },
    KeywordEntry { text: "typeof", detail: "keyword: type query", snippet: None },
    KeywordEntry { text: "not", detail: "keyword: boolean negation", snippet: None },
    KeywordEntry { text: "import", detail: "keyword: module import", snippet: Some("import ${1:module}") },
    KeywordEntry { text: "export", detail: "keyword: module export", snippet: None },
    KeywordEntry { text: "module", detail: "keyword: module declaration", snippet: Some("module ${1:name} { ${2:body} }") },
    KeywordEntry { text: "const", detail: "keyword: constant binding", snippet: Some("const ${1:name} = ${2:value}") },
    KeywordEntry { text: "is", detail: "keyword: type check", snippet: None },
    KeywordEntry { text: "may", detail: "keyword: capability (may)", snippet: None },
    KeywordEntry { text: "must", detail: "keyword: capability (must)", snippet: None },
    KeywordEntry { text: "when", detail: "keyword: when-guard", snippet: None },
    KeywordEntry { text: "do", detail: "keyword: do-block", snippet: None },
    KeywordEntry { text: "true", detail: "boolean literal", snippet: None },
    KeywordEntry { text: "false", detail: "boolean literal", snippet: None },
    KeywordEntry { text: "null", detail: "null literal", snippet: None },
];

/// Arrow combinator snippets — useful inside arrow-graph expressions.
const ARROW_SNIPPETS: &[(&str, &str, &str)] = &[
    ("arr", "arrow: arr(name) — lift native op", "arr(\"${1:op}\")"),
    ("comp", "arrow: comp(a, b) — sequential composition", "comp(${1:a}, ${2:b})"),
    ("first", "arrow: first(a) — process first of pair", "first(${1:a})"),
    ("choice", "arrow: choice(l, r) — branch", "choice(${1:l}, ${2:r})"),
    ("app", "arrow: app(name, args) — call named arrow", "app(\"${1:name}\", ${2:args})"),
    ("id", "arrow: identity", "id"),
];

/// All known arrow combinators, offered when the cursor sits at a position
/// where an arrow expression is expected (after `=`, `<-`, or at the start of
/// a line inside a `do {}` block). Includes the Paterson-notation terminators
/// `loop`, `insert`, and `returnA` alongside the pure combinators.
const COMBINATORS: &[(&str, &str, &str)] = &[
    ("arr", "arrow: arr(name) — lift native op", "arr(\"${1:op}\")"),
    ("comp", "arrow: comp(a, b) — sequential composition", "comp(${1:a}, ${2:b})"),
    ("first", "arrow: first(a) — process first of pair", "first(${1:a})"),
    ("choice", "arrow: choice(l, r) — branch", "choice(${1:l}, ${2:r})"),
    ("app", "arrow: app(name, args) — call named arrow", "app(\"${1:name}\", ${2:args})"),
    ("loop", "arrow: loop(state) — feedback loop", "loop { ${1:state} }"),
    ("id", "arrow: identity", "id"),
    ("insert", "arrow: insert(value) — inject a value", "insert(${1:value})"),
    ("returnA", "arrow: returnA — terminate with a value", "returnA -< ${1:expr}"),
];

/// Classify an identifier token by its syntactic role, returning
/// `(kind, detail)` for the completion item.
fn classify_identifier(tokens: &[SpannedToken], index: usize) -> (i64, &'static str) {
    let is_kind = |i: usize, k: &str| -> bool {
        tokens.get(i).map(|t| t.kind == k).unwrap_or(false)
    };

    // Property access: preceded by `.` → property or method
    if index > 0 && is_kind(index - 1, "token.dot") {
        if is_kind(index + 1, "token.lparen") {
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
    if ident == "type" && is_kind(index + 1, "token.dot") {
        return (KIND_CLASS, "type");
    }

    // Function call: next token is `(`
    if is_kind(index + 1, "token.lparen") {
        return (KIND_FUNCTION, "function");
    }

    // Dotted call: `ident . ident (`
    if is_kind(index + 1, "token.dot")
        && tokens
            .get(index + 2)
            .map(|t| is_identifier_kind(&t.kind))
            .unwrap_or(false)
        && is_kind(index + 3, "token.lparen")
    {
        return (KIND_FUNCTION, "function");
    }

    // `key:` inside parens → parameter; inside braces → property
    if is_kind(index + 1, "token.colon") {
        return match enclosing_open_delim(tokens, index) {
            Some("token.lparen") => (KIND_PARAMETER, "parameter"),
            Some("token.lbrace") => (KIND_PROPERTY, "property"),
            _ => (KIND_VARIABLE, "binding"),
        };
    }

    // Binding declaration: at line start, followed by `:` or `=`
    if is_kind(index + 1, "token.colon") || is_kind(index + 1, "token.equal") {
        return (KIND_VARIABLE, "binding");
    }

    (KIND_VARIABLE, "variable")
}

/// Find the nearest unmatched opening delimiter enclosing `index`.
fn enclosing_open_delim(tokens: &[SpannedToken], index: usize) -> Option<&'static str> {
    let mut depth_paren: i32 = 0;
    let mut depth_brace: i32 = 0;
    let mut i = index;
    while i > 0 {
        i -= 1;
        match tokens[i].kind.as_str() {
            "token.rparen" => depth_paren += 1,
            "token.rbrace" => depth_brace += 1,
            "token.lparen" if depth_paren == 0 => return Some("token.lparen"),
            "token.lparen" => depth_paren -= 1,
            "token.lbrace" if depth_brace == 0 => return Some("token.lbrace"),
            "token.lbrace" => depth_brace -= 1,
            _ => {}
        }
    }
    None
}

/// Detect whether the cursor is inside an arrow-graph expression (i.e. the
/// file looks like an arrow file). If so, arrow combinator snippets are
/// offered alongside keywords.
fn looks_like_arrow_file(source: &str) -> bool {
    let trimmed = source.trim_start();
    trimmed.starts_with("loop {")
        || trimmed.starts_with("comp(")
        || trimmed.starts_with("arr(")
        || trimmed.starts_with("first(")
        || trimmed.starts_with("choice(")
        || trimmed.starts_with("app(")
        || trimmed.starts_with("id")
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
    if let Ok(kit) = Kit::new() {
        if let Ok(tokens) = kit.lex_with_spans(source) {
            // Collect unique names with their best (most specific) role.
            // Prefer function > type > parameter/property > binding > variable.
            let mut seen: BTreeMap<String, (i64, &'static str)> = BTreeMap::new();
            for (i, token) in tokens.as_slice().iter().enumerate() {
                if !is_identifier_kind(&token.kind) {
                    continue;
                }
                let role = classify_identifier(&tokens, i);
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
    /// Cursor right after the `loop` keyword — suggest the state block `{ ... }`.
    AfterLoop,
    /// Cursor right after the `proc` keyword — suggest `(state) -> body`.
    AfterProc,
    /// Cursor right after the `match` keyword — suggest `state.field { ... }`.
    AfterMatch,
    /// Cursor right after the `do` keyword — suggest the block `{ ... }`.
    AfterDo,
    /// Cursor right after the `returnA` keyword — suggest `-< expr`.
    AfterReturnA,
    /// Cursor right after `state.` (or after the `{` of a loop state block) —
    /// suggest the field names declared in the loop state.
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
fn enclosing_lbrace_index_before(tokens: &[SpannedToken], offset: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = tokens.len();
    while i > 0 {
        i -= 1;
        if tokens[i].span.start >= offset {
            continue;
        }
        match tokens[i].kind.as_str() {
            "token.rbrace" => depth += 1,
            "token.lbrace" if depth == 0 => return Some(i),
            "token.lbrace" => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Whether a given keyword text appears immediately before a `{` brace (with
/// no intervening nested braces). Used to recognise `match … {` and `do {`
/// blocks by inspecting the brace's opening token.
fn keyword_before_brace(tokens: &[SpannedToken], brace_idx: usize, kw: &str) -> bool {
    let mut depth: i32 = 0;
    let mut j = brace_idx;
    while j > 0 {
        j -= 1;
        match tokens[j].kind.as_str() {
            "token.rbrace" => depth += 1,
            "token.lbrace" => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            _ if depth == 0 && is_keyword_kind(&tokens[j].kind) && tokens[j].text == kw => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Whether the cursor sits inside a `do { … }` block.
fn inside_do_block(tokens: &[SpannedToken], offset: usize) -> bool {
    enclosing_lbrace_index_before(tokens, offset)
        .map(|lb| keyword_before_brace(tokens, lb, "do"))
        .unwrap_or(false)
}

/// Whether the colon at `colon_idx` is a pattern colon inside a `match` block
/// (i.e. the token preceding the colon is `]` and the enclosing brace belongs
/// to a `match` expression).
fn is_match_pattern_colon(tokens: &[SpannedToken], colon_idx: usize) -> bool {
    if colon_idx == 0 {
        return false;
    }
    if tokens[colon_idx - 1].kind != "token.rbracket" {
        return false;
    }
    let offset = tokens[colon_idx].span.end;
    enclosing_lbrace_index_before(tokens, offset)
        .map(|lb| keyword_before_brace(tokens, lb, "match"))
        .unwrap_or(false)
}

/// Extract the field names declared in a loop state block
/// (`loop { name: value, … }`). Scans every `loop` keyword in the source and
/// collects the identifiers that are immediately followed by `:` inside the
/// state block's braces.
fn loop_state_fields(tokens: &[SpannedToken]) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let is_loop = is_keyword_kind(&tokens[i].kind) && tokens[i].text == "loop";
        let next_is_brace = tokens.get(i + 1).map(|t| t.kind == "token.lbrace").unwrap_or(false);
        if is_loop && next_is_brace {
            let mut j = i + 2;
            let mut depth: i32 = 1;
            while j < tokens.len() && depth > 0 {
                match tokens[j].kind.as_str() {
                    "token.lbrace" => depth += 1,
                    "token.rbrace" => depth -= 1,
                    _ => {}
                }
                if depth > 0 && is_identifier_kind(&tokens[j].kind) {
                    let followed_by_colon = tokens
                        .get(j + 1)
                        .map(|t| t.kind == "token.colon")
                        .unwrap_or(false);
                    if followed_by_colon {
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
fn detect_context(tokens: &[SpannedToken], offset: usize, source: &str) -> Context {
    let idx = match preceding_index(tokens, offset) {
        Some(i) => i,
        None => {
            // Cursor before any token: only suggest combinators if we are
            // inside a `do {}` block (start of a statement).
            return if inside_do_block(tokens, offset) {
                Context::ArrowExpr
            } else {
                Context::None
            };
        }
    };
    let kind = tokens[idx].kind.as_str();
    let text = tokens[idx].text.as_str();

    // 1. Right after a structural keyword.
    if is_keyword_kind(kind) {
        match text {
            "loop" => return Context::AfterLoop,
            "proc" => return Context::AfterProc,
            "match" => return Context::AfterMatch,
            "do" => return Context::AfterDo,
            "returnA" => return Context::AfterReturnA,
            _ => {}
        }
    }

    // 2. Field access `state.` → suggest in-scope loop state fields.
    if kind == "token.dot"
        && idx > 0
        && is_identifier_kind(&tokens[idx - 1].kind)
        && tokens[idx - 1].text == "state"
    {
        return Context::StateField;
    }

    // 2b. Right after the `{` opening a loop state block → field names.
    if kind == "token.lbrace"
        && idx > 0
        && is_keyword_kind(&tokens[idx - 1].kind)
        && tokens[idx - 1].text == "loop"
    {
        return Context::StateField;
    }

    // 3. After a pattern colon inside a `match` block → list patterns.
    if kind == "token.colon" && is_match_pattern_colon(tokens, idx) {
        return Context::MatchPattern;
    }

    // 4. Arrow expression position: after `<-` (always arrow-specific) or
    //    after `=` when in an arrow context.
    if kind == "token.arrow_bind" {
        return Context::ArrowExpr;
    }
    if kind == "token.equal" && (looks_like_arrow_file(source) || inside_do_block(tokens, offset))
    {
        return Context::ArrowExpr;
    }

    // 4b. Start of a new line inside a `do {}` block (after `;`, `}`, or the
    //     block's opening `{`) → arrow expression position.
    if (kind == "token.semicolon" || kind == "token.rbrace" || kind == "token.lbrace")
        && inside_do_block(tokens, offset)
        && !(kind == "token.lbrace"
            && idx > 0
            && is_keyword_kind(&tokens[idx - 1].kind)
            && tokens[idx - 1].text == "loop")
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

    let tokens = match Kit::new().and_then(|kit| kit.lex_with_spans(source)) {
        Ok(t) => t,
        Err(_) => return json!(items),
    };

    match detect_context(&tokens, offset, source) {
        Context::None => {}
        Context::AfterLoop => items.push(json!({
            "label": "{",
            "kind": KIND_SNIPPET,
            "detail": "loop state block",
            "insertText": "{ ${1:remaining}: ${2:input}, ${3:acc}: [] }",
            "insertTextFormat": 2,
        })),
        Context::AfterProc => items.push(json!({
            "label": "(state) ->",
            "kind": KIND_SNIPPET,
            "detail": "proc parameter and arrow body",
            "insertText": "(${1:state}) -> ${2:body}",
            "insertTextFormat": 2,
        })),
        Context::AfterMatch => items.push(json!({
            "label": "state.",
            "kind": KIND_SNIPPET,
            "detail": "match on a state field",
            "insertText": "state.${1:field} {\n  []: { out: ${2:acc} },\n  [h, ...t]: { state: ${3:next} }\n}",
            "insertTextFormat": 2,
        })),
        Context::AfterDo => items.push(json!({
            "label": "{",
            "kind": KIND_SNIPPET,
            "detail": "do-block body",
            "insertText": "{\n  ${1:stmts}\n}",
            "insertTextFormat": 2,
        })),
        Context::AfterReturnA => items.push(json!({
            "label": "-<",
            "kind": KIND_SNIPPET,
            "detail": "returnA input expression",
            "insertText": "-< ${1:expr}",
            "insertTextFormat": 2,
        })),
        Context::StateField => {
            for field in loop_state_fields(&tokens) {
                items.push(json!({
                    "label": field,
                    "kind": KIND_PROPERTY,
                    "detail": "loop state field",
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
