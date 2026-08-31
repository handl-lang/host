use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

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
