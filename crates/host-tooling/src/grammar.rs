use handl_language::{Language, LexerPattern, LexerRuleDef};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

// ---------------------------------------------------------------------------
// Data-driven grammar generation (ADR-0044)
// ---------------------------------------------------------------------------

/// Grammar-rule priorities (lower = earlier in the patterns array, i.e.
/// first-match-wins).  Categories are ordered so that more specific patterns
/// (comments, keywords) appear before less specific ones (identifiers).
const PRIO_DOC_COMMENT: i32 = 10;
const PRIO_COMMENT: i32 = 11;
const PRIO_STRING: i32 = 15;
const PRIO_NUMBER: i32 = 20;
const PRIO_BOOLEAN: i32 = 25;
const PRIO_NULL: i32 = 26;
const PRIO_KEYWORD: i32 = 30;
const PRIO_OPERATOR: i32 = 40;
const PRIO_DELIMITER: i32 = 50;
const PRIO_TYPE_NAME: i32 = 90;
const PRIO_IDENTIFIER: i32 = 100;

/// Token category derived from the lexer-rule's `token_kind` and pattern.
enum TokenCategory {
    DocComment,
    LineComment,
    BlockComment,
    String,
    Number,
    Boolean,
    Null,
    Keyword,
    Operator,
    Delimiter,
    Identifier,
    Other,
}

/// Classify a lexer rule into a [`TokenCategory`].
fn classify_token(kind: &str, pattern: &LexerPattern) -> TokenCategory {
    match kind {
        "token.comment" => {
            if let LexerPattern::Delimited { close_at_eof: true, .. } = pattern {
                TokenCategory::LineComment
            } else {
                TokenCategory::BlockComment
            }
        }
        "token.doc_comment" => {
            if let LexerPattern::Delimited { close_at_eof: true, .. } = pattern {
                TokenCategory::DocComment
            } else {
                TokenCategory::BlockComment
            }
        }
        "token.string" | "token.template" | "token.json" => TokenCategory::String,
        "token.number" => TokenCategory::Number,
        "token.bool" => TokenCategory::Boolean,
        "token.null" => TokenCategory::Null,
        "token.identifier" => TokenCategory::Identifier,
        _ => {
            // Keyword token kinds.
            if kind.starts_with("token.keyword.") {
                return TokenCategory::Keyword;
            }
            // Structural delimiters.
            if is_structural_delimiter(kind) {
                return TokenCategory::Delimiter;
            }
            // Literal patterns with identifier-like text are keywords
            // (e.g. `token.let` → "let", `token.vau` → "vau").
            if let LexerPattern::Literal { text } = pattern {
                if text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && text
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_alphabetic() || c == '_')
                        .unwrap_or(false)
                {
                    return TokenCategory::Keyword;
                }
            }
            // Everything else with a literal pattern is an operator.
            if matches!(pattern, LexerPattern::Literal { .. }) {
                TokenCategory::Operator
            } else {
                TokenCategory::Other
            }
        }
    }
}

/// Whether a token kind is a structural delimiter (parens, brackets, braces,
/// comma, semicolon, colon).
fn is_structural_delimiter(kind: &str) -> bool {
    matches!(
        kind,
        "token.lparen"
            | "token.rparen"
            | "token.lbracket"
            | "token.rbracket"
            | "token.lbrace"
            | "token.rbrace"
            | "token.comma"
            | "token.semicolon"
            | "token.colon"
    )
}

/// Escape regex metacharacters in a literal string.
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for ch in s.chars() {
        if "\\^$.|?*+()[]{}".contains(ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Convert a lexer character-class spec (e.g. `"a-zA-Z_"`) into a regex
/// character class (e.g. `"[a-zA-Z_]"`).
fn char_class_to_regex(chars: &str) -> String {
    let mut out = String::with_capacity(chars.len() + 2);
    out.push('[');
    for ch in chars.chars() {
        if ch == ']' || ch == '\\' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push(']');
    out
}

/// Convert a data-driven [`Language`] into a list of [`GrammarRule`]s suitable
/// for [`generate_textmate`].
///
/// Rules are derived from the language's lexer rules, grouped by token
/// category (comments, strings, numbers, booleans, nulls, keywords, operators,
/// delimiters, type names, identifiers) and ordered by priority so that more
/// specific patterns appear before less specific ones.
///
/// This is the data-driven replacement for the former `kit.grammar_rules()`
/// method that collected rules from Rust-typed token factories (removed in the
/// ADR-0044 emitter migration).
pub fn grammar_rules_from_language(lang: &Language) -> Vec<GrammarRule> {
    // Sort lexer rules by priority (lower = tried first by the lexer engine).
    let mut lex_rules: Vec<&LexerRuleDef> = lang.lexer_rules.iter().collect();
    lex_rules.sort_by_key(|r| r.priority);

    // Collect literal texts for grouped rules.
    let mut kw_texts: Vec<String> = Vec::new();
    let mut op_texts: Vec<String> = Vec::new();
    let mut delim_texts: Vec<String> = Vec::new();
    let mut bool_texts: Vec<String> = Vec::new();
    let mut has_null = false;

    let mut result: Vec<GrammarRule> = Vec::new();

    for rule in &lex_rules {
        let kind = rule.token_kind.as_str();
        let category = classify_token(kind, &rule.pattern);

        match category {
            TokenCategory::DocComment | TokenCategory::LineComment => {
                if let LexerPattern::Delimited { open, .. } = &rule.pattern {
                    let is_doc = matches!(category, TokenCategory::DocComment);
                    let name = if is_doc { "doc_comments" } else { "comments" };
                    let scope = if is_doc {
                        "comment.documentation"
                    } else {
                        "comment.line"
                    };
                    let prio = if is_doc {
                        PRIO_DOC_COMMENT
                    } else {
                        PRIO_COMMENT
                    };
                    let pattern = format!("{}.*", regex_escape(open));
                    result.push(GrammarRule::match_rule(name, scope, &pattern, prio));
                }
            }
            TokenCategory::BlockComment => {
                if let LexerPattern::Delimited { open, close, .. } = &rule.pattern {
                    result.push(GrammarRule::begin_end_rule(
                        "block_comment",
                        "comment.block",
                        &regex_escape(open),
                        &regex_escape(close),
                        PRIO_COMMENT,
                    ));
                }
            }
            TokenCategory::String => {
                if let LexerPattern::Delimited { open, close, .. } = &rule.pattern {
                    let (name, scope) = if kind == "token.template" {
                        ("templates", "string.template")
                    } else if kind == "token.json" {
                        ("json_blocks", "string.interpolated")
                    } else {
                        ("strings", "string.quoted.double")
                    };
                    result.push(GrammarRule::begin_end_rule(
                        name,
                        scope,
                        &regex_escape(open),
                        &regex_escape(close),
                        PRIO_STRING,
                    ));
                }
            }
            TokenCategory::Number => {
                result.push(GrammarRule::match_rule(
                    "numbers",
                    "constant.numeric",
                    r"-?\d+(\.\d+)?",
                    PRIO_NUMBER,
                ));
            }
            TokenCategory::Boolean => {
                if let LexerPattern::Literal { text } = &rule.pattern {
                    bool_texts.push(text.clone());
                }
            }
            TokenCategory::Null => {
                has_null = true;
            }
            TokenCategory::Keyword => {
                if let LexerPattern::Literal { text } = &rule.pattern {
                    kw_texts.push(text.clone());
                }
            }
            TokenCategory::Operator => {
                if let LexerPattern::Literal { text } = &rule.pattern {
                    op_texts.push(text.clone());
                }
            }
            TokenCategory::Delimiter => {
                if let LexerPattern::Literal { text } = &rule.pattern {
                    delim_texts.push(text.clone());
                }
            }
            TokenCategory::Identifier => {
                if let LexerPattern::CharClass {
                    start_chars,
                    continue_chars,
                } = &rule.pattern
                {
                    let pattern = format!(
                        "{}{}*",
                        char_class_to_regex(start_chars),
                        char_class_to_regex(continue_chars)
                    );
                    result.push(GrammarRule::match_rule(
                        "identifiers",
                        "variable.other.readwrite",
                        &pattern,
                        PRIO_IDENTIFIER,
                    ));
                }
            }
            TokenCategory::Other => {}
        }
    }

    // --- Grouped rules (multiple literals collapsed into one pattern) ---

    if !bool_texts.is_empty() {
        bool_texts.sort();
        bool_texts.dedup();
        let pattern = format!(r"\b({})\b", bool_texts.join("|"));
        result.push(GrammarRule::match_rule(
            "booleans",
            "constant.language.boolean",
            &pattern,
            PRIO_BOOLEAN,
        ));
    }

    if has_null {
        result.push(GrammarRule::match_rule(
            "nulls",
            "constant.language.null",
            r"\b(null)\b",
            PRIO_NULL,
        ));
    }

    if !kw_texts.is_empty() {
        kw_texts.sort();
        kw_texts.dedup();
        let pattern = format!(r"\b({})\b", kw_texts.join("|"));
        result.push(GrammarRule::match_rule(
            "keywords",
            "keyword.control",
            &pattern,
            PRIO_KEYWORD,
        ));
    }

    if !op_texts.is_empty() {
        // Sort by length descending so longer operators match first.
        op_texts.sort_by(|a, b| b.len().cmp(&a.len()));
        op_texts.dedup();
        let escaped: Vec<String> = op_texts.iter().map(|t| regex_escape(t)).collect();
        let pattern = format!("({})", escaped.join("|"));
        result.push(GrammarRule::match_rule(
            "operators",
            "keyword.operator",
            &pattern,
            PRIO_OPERATOR,
        ));
    }

    if !delim_texts.is_empty() {
        delim_texts.sort();
        delim_texts.dedup();
        let all_single = delim_texts.iter().all(|t| t.chars().count() == 1);
        let pattern = if all_single {
            let mut cls = String::from("[");
            for t in &delim_texts {
                for ch in t.chars() {
                    if ch == ']' || ch == '\\' || ch == '^' || ch == '-' {
                        cls.push('\\');
                    }
                    cls.push(ch);
                }
            }
            cls.push(']');
            cls
        } else {
            let escaped: Vec<String> = delim_texts.iter().map(|t| regex_escape(t)).collect();
            format!("({})", escaped.join("|"))
        };
        result.push(GrammarRule::match_rule(
            "delimiters",
            "punctuation.separator",
            &pattern,
            PRIO_DELIMITER,
        ));
    }

    // Type names (derived convention): capitalized identifiers.
    result.push(GrammarRule::match_rule(
        "type_names",
        "entity.name.type",
        r"\b[A-Z][a-zA-Z0-9_]*\b",
        PRIO_TYPE_NAME,
    ));

    // Sort by priority (lower = earlier in the patterns array).
    result.sort_by_key(|r| r.priority);
    result
}

/// A single TextMate grammar rule.
///
/// Each rule describes how a token category should be highlighted in the
/// generated TextMate grammar. Rules are sorted by `priority` (lower =
/// earlier in the patterns array) and passed to [`generate_textmate`] to
/// produce the final `.tmLanguage.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrammarRule {
    /// Repository key (e.g. "keywords", "numbers", "function_calls").
    pub name: String,
    /// TextMate scope name (e.g. "keyword", "constant.numeric", "variable").
    /// Applied to the entire match, or to the begin/end block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Single-line match pattern. Mutually exclusive with `begin`/`end`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#match: Option<String>,
    /// Begin pattern for multi-line constructs (strings, block comments).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub begin: Option<String>,
    /// End pattern for multi-line constructs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// Capture group → scope mappings.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub captures: Vec<GrammarCapture>,
    /// Nested patterns inside begin/end blocks.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub patterns: Vec<GrammarRule>,
    /// Priority — lower = earlier in the patterns array (first-match-wins).
    pub priority: i32,
}

impl GrammarRule {
    /// Create a simple match rule with a scope.
    pub fn match_rule(name: &str, scope: &str, pattern: &str, priority: i32) -> Self {
        Self {
            name: name.to_string(),
            scope: Some(scope.to_string()),
            r#match: Some(pattern.to_string()),
            begin: None,
            end: None,
            captures: Vec::new(),
            patterns: Vec::new(),
            priority,
        }
    }

    /// Create a begin/end rule for multi-line constructs.
    pub fn begin_end_rule(name: &str, scope: &str, begin: &str, end: &str, priority: i32) -> Self {
        Self {
            name: name.to_string(),
            scope: Some(scope.to_string()),
            r#match: None,
            begin: Some(begin.to_string()),
            end: Some(end.to_string()),
            captures: Vec::new(),
            patterns: Vec::new(),
            priority,
        }
    }

    /// Create a match rule with captures (for sub-group scoping).
    pub fn match_with_captures(
        name: &str,
        pattern: &str,
        captures: Vec<GrammarCapture>,
        priority: i32,
    ) -> Self {
        Self {
            name: name.to_string(),
            scope: None,
            r#match: Some(pattern.to_string()),
            begin: None,
            end: None,
            captures,
            patterns: Vec::new(),
            priority,
        }
    }
}

/// A capture group mapping within a [`GrammarRule`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrammarCapture {
    /// Capture group number (1-indexed).
    pub group: u32,
    /// TextMate scope for this capture group.
    pub scope: String,
}

impl GrammarCapture {
    /// Create a new capture group mapping.
    pub fn new(group: u32, scope: &str) -> Self {
        Self {
            group,
            scope: scope.to_string(),
        }
    }
}

/// Generate a TextMate grammar JSON from a list of [`GrammarRule`]s.
///
/// Rules are already sorted by priority (lower = earlier in patterns).
/// The output is a `.tmLanguage.json`-compatible JSON value.
pub fn generate_textmate(
    rules: &[GrammarRule],
    scope_name: &str,
    file_extensions: &[String],
) -> Value {
    let _ = file_extensions;
    let mut repository = Map::new();
    let mut patterns: Vec<Value> = Vec::new();

    for rule in rules {
        repository.insert(rule.name.clone(), rule_to_json(rule));
        patterns.push(json!({ "include": format!("#{}", rule.name) }));
    }

    json!({
        "scopeName": scope_name,
        "patterns": patterns,
        "repository": repository,
    })
}

/// Convert a single GrammarRule to its TextMate JSON representation.
fn rule_to_json(rule: &GrammarRule) -> Value {
    let mut obj = Map::new();

    if let Some(ref scope) = rule.scope {
        obj.insert("name".into(), json!(scope));
    }
    if let Some(ref m) = rule.r#match {
        obj.insert("match".into(), json!(m));
    }
    if let Some(ref begin) = rule.begin {
        obj.insert("begin".into(), json!(begin));
    }
    if let Some(ref end) = rule.end {
        obj.insert("end".into(), json!(end));
    }

    if !rule.captures.is_empty() {
        let captures: Map<String, Value> = rule
            .captures
            .iter()
            .map(|c| (c.group.to_string(), json!({ "name": c.scope })))
            .collect();
        obj.insert("captures".into(), json!(captures));
    }

    if !rule.patterns.is_empty() {
        let nested: Vec<Value> = rule.patterns.iter().map(rule_to_json).collect();
        obj.insert("patterns".into(), json!(nested));
    }

    Value::Object(obj)
}
