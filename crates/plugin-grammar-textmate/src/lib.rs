use serde::{Deserialize, Serialize};

/// A single TextMate grammar rule contributed by a token plugin.
///
/// Each token plugin that wants to contribute to the generated TextMate
/// grammar implements [`GrammarContributor`] and returns one or more
/// `GrammarRule` values. The kit collects all contributed rules, sorts
/// them by `priority` (lower = earlier in the patterns array), and passes
/// them to `generate_textmate` to produce the final `.tmLanguage.json`.
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
    pub fn new(group: u32, scope: &str) -> Self {
        Self {
            group,
            scope: scope.to_string(),
        }
    }
}

/// Trait that token plugins implement to declare their grammar rules.
///
/// Each plugin returns a list of [`GrammarRule`] values describing how
/// its tokens should be highlighted in the generated TextMate grammar.
/// The kit collects all contributed rules and passes them to the
/// grammar generator.
pub trait GrammarContributor {
    fn grammar_rules(&self) -> Vec<GrammarRule>;
}

/// Collect grammar rules from multiple contributors, sorted by priority.
pub fn collect_rules(contributors: &[&dyn GrammarContributor]) -> Vec<GrammarRule> {
    let mut rules: Vec<GrammarRule> = contributors
        .iter()
        .flat_map(|c| c.grammar_rules())
        .collect();
    rules.sort_by_key(|r| r.priority);
    rules
}
