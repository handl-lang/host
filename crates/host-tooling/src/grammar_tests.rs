use crate::{GrammarCapture, GrammarRule, generate_textmate, grammar_rules_from_language};

fn sample_rules() -> Vec<GrammarRule> {
    vec![
        GrammarRule::match_rule("comments", "comment.line", r"//.*", 20),
        GrammarRule::begin_end_rule("block_comment", "comment.block", r"/\*", r"\*/", 21),
        GrammarRule::match_rule("numbers", "constant.numeric", r"-?\d+(\.\d+)?", 30),
        GrammarRule::match_rule("keywords", "keyword", r"\b(let|const)\b", 50),
        GrammarRule::match_rule(
            "booleans",
            "constant.language.boolean",
            r"\b(true|false)\b",
            40,
        ),
        GrammarRule::match_rule("nulls", "constant.language.null", r"\b(null)\b", 41),
        GrammarRule::match_rule("operators", "keyword.operator", r"(\+|~>)", 55),
        GrammarRule::match_rule("delimiters", "punctuation", r"[\(\)]", 58),
        GrammarRule::match_with_captures(
            "function_calls",
            r"([a-zA-Z_][a-zA-Z0-9_]*)\s*(\()",
            vec![
                GrammarCapture::new(1, "entity.name.function"),
                GrammarCapture::new(2, "punctuation"),
            ],
            70,
        ),
        GrammarRule::match_rule("identifiers", "variable", r"[a-zA-Z_][a-zA-Z0-9_]*", 100),
    ]
}

#[test]
fn generate_textmate_produces_valid_json() {
    let rules = sample_rules();
    let grammar = generate_textmate(&rules, "source.test", &["test".to_string()]);
    assert_eq!(grammar["scopeName"], "source.test");
    assert!(grammar["patterns"].is_array());
    assert!(grammar["repository"].is_object());
}

#[test]
fn generate_textmate_includes_all_rules() {
    let rules = sample_rules();
    let grammar = generate_textmate(&rules, "source.test", &["test".to_string()]);
    let repo = &grammar["repository"];
    assert!(repo.get("comments").is_some());
    assert!(repo.get("numbers").is_some());
    assert!(repo.get("keywords").is_some());
    assert!(repo.get("function_calls").is_some());
    assert!(repo.get("identifiers").is_some());
}

#[test]
fn generate_textmate_patterns_in_priority_order() {
    let rules = sample_rules();
    let grammar = generate_textmate(&rules, "source.test", &["test".to_string()]);
    let patterns = grammar["patterns"].as_array().unwrap();
    let includes: Vec<&str> = patterns
        .iter()
        .filter_map(|p| p["include"].as_str())
        .collect();
    // comments (priority 20) should come before keywords (priority 50)
    let comments_pos = includes.iter().position(|s| *s == "#comments").unwrap();
    let keywords_pos = includes.iter().position(|s| *s == "#keywords").unwrap();
    assert!(comments_pos < keywords_pos);
    // function_calls (priority 70) should come before identifiers (priority 100)
    let fc_pos = includes
        .iter()
        .position(|s| *s == "#function_calls")
        .unwrap();
    let id_pos = includes.iter().position(|s| *s == "#identifiers").unwrap();
    assert!(fc_pos < id_pos);
}

#[test]
fn generate_textmate_function_calls_has_captures() {
    let rules = sample_rules();
    let grammar = generate_textmate(&rules, "source.test", &["test".to_string()]);
    let fc = &grammar["repository"]["function_calls"];
    assert!(fc["match"].as_str().unwrap().contains(r"\("));
    let captures = &fc["captures"];
    assert_eq!(captures["1"]["name"], "entity.name.function");
    assert_eq!(captures["2"]["name"], "punctuation");
}

#[test]
fn generate_textmate_begin_end_rule() {
    let rules = sample_rules();
    let grammar = generate_textmate(&rules, "source.test", &["test".to_string()]);
    let bc = &grammar["repository"]["block_comment"];
    assert_eq!(bc["begin"], r"/\*");
    assert_eq!(bc["end"], r"\*/");
    assert_eq!(bc["name"], "comment.block");
}

#[test]
fn generate_textmate_empty_rules() {
    let grammar = generate_textmate(&[], "source.empty", &["empty".to_string()]);
    assert_eq!(grammar["scopeName"], "source.empty");
    assert_eq!(grammar["patterns"].as_array().unwrap().len(), 0);
}

// --- Data-driven grammar generation tests (ADR-0044) ---

fn canonical_rules() -> Vec<GrammarRule> {
    let kit = handl_kit_canonical::Kit::new().unwrap();
    grammar_rules_from_language(kit.runtime().lang())
}

#[test]
fn grammar_rules_from_language_produces_non_empty_rules() {
    let rules = canonical_rules();
    assert!(!rules.is_empty(), "grammar rules should not be empty");
}

#[test]
fn grammar_rules_from_language_includes_all_categories() {
    let rules = canonical_rules();
    let names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();
    for required in [
        "comments",
        "doc_comments",
        "strings",
        "numbers",
        "booleans",
        "nulls",
        "keywords",
        "operators",
        "delimiters",
        "type_names",
        "identifiers",
    ] {
        assert!(
            names.contains(&required),
            "missing grammar category: {required}"
        );
    }
}

#[test]
fn grammar_rules_from_language_keywords_include_required() {
    let rules = canonical_rules();
    let kw_rule = rules
        .iter()
        .find(|r| r.name == "keywords")
        .expect("keywords rule exists");
    let pattern = kw_rule.r#match.as_ref().expect("keywords has a match pattern");
    for required in [
        "loop", "if", "then", "else", "match", "where", "in", "typeof", "not", "import",
        "export", "module", "axiom", "as", "handle",
    ] {
        assert!(
            pattern.contains(required),
            "keywords pattern missing: {required}"
        );
    }
}

#[test]
fn grammar_rules_from_language_generates_valid_grammar() {
    let rules = canonical_rules();
    let grammar = generate_textmate(&rules, "source.handl", &["hnl".to_string()]);
    assert_eq!(grammar["scopeName"], "source.handl");
    let patterns = grammar["patterns"].as_array().unwrap();
    assert!(!patterns.is_empty(), "patterns should not be empty");
    let repo = &grammar["repository"];
    assert!(repo.get("keywords").is_some());
    assert!(repo.get("comments").is_some());
    assert!(repo.get("identifiers").is_some());
}
