use serde_json::{Map, Value, json};

pub use handl_plugin_grammar_textmate::{GrammarRule, GrammarCapture, GrammarContributor, collect_rules};

/// Generate a TextMate grammar JSON from a list of [`GrammarRule`]s.
///
/// Rules are already sorted by priority (lower = earlier in patterns).
/// The output is a `.tmLanguage.json`-compatible JSON value.
pub fn generate_textmate(rules: &[GrammarRule], scope_name: &str, file_extensions: &[String]) -> Value {
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
