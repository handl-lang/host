use crate::{GrammarInfo, generate_textmate};

fn sample_info() -> GrammarInfo {
    GrammarInfo {
        scope_name: "source.test".to_string(),
        file_extensions: vec!["test".to_string()],
        keywords: vec!["let".to_string(), "const".to_string()],
        booleans: vec!["true".to_string(), "false".to_string()],
        nulls: vec!["null".to_string()],
        operators: vec!["+".to_string(), "->".to_string()],
        delimiters: vec!["(".to_string(), ")".to_string()],
        line_comment_start: Some("//".to_string()),
        block_comment: Some(("/*".to_string(), "*/".to_string())),
        doc_line_comment_start: Some("///".to_string()),
        doc_block_comment: Some(("/**".to_string(), "*/".to_string())),
        string_delimiters: vec!['"', '\''],
        number_regex: r"-?\d+(\.\d+)?".to_string(),
        identifier_regex: r"[a-zA-Z_][a-zA-Z0-9_]*".to_string(),
    }
}

#[test]
fn generate_textmate_produces_valid_json() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    assert_eq!(grammar["scopeName"], "source.test");
    assert!(grammar["patterns"].is_array());
    assert!(grammar["repository"].is_object());
}

#[test]
fn generate_textmate_includes_keywords() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    assert!(repo["keywords"]["match"].as_str().unwrap().contains("let"));
    assert!(repo["keywords"]["match"].as_str().unwrap().contains("const"));
}

#[test]
fn generate_textmate_includes_booleans() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    assert!(repo["booleans"]["match"].as_str().unwrap().contains("true"));
    assert!(repo["booleans"]["match"].as_str().unwrap().contains("false"));
}

#[test]
fn generate_textmate_includes_nulls() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    assert!(repo["nulls"]["match"].as_str().unwrap().contains("null"));
}

#[test]
fn generate_textmate_includes_operators() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    let op_match = repo["operators"]["match"].as_str().unwrap();
    assert!(op_match.contains("+"));
    assert!(op_match.contains("->"));
}

#[test]
fn generate_textmate_includes_delimiters() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    let delim_match = repo["delimiters"]["match"].as_str().unwrap();
    assert!(delim_match.contains("("));
    assert!(delim_match.contains(")"));
}

#[test]
fn generate_textmate_includes_comments() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    assert!(repo.get("comments").is_some());
    assert!(repo.get("block_comment").is_some());
    assert!(repo.get("doc_comments").is_some());
    assert!(repo.get("doc_block_comment").is_some());
}

#[test]
fn generate_textmate_includes_strings() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    assert!(repo.get("strings").is_some());
    let patterns = repo["strings"]["patterns"].as_array().unwrap();
    assert_eq!(patterns.len(), 2);
}

#[test]
fn generate_textmate_includes_numbers_and_identifiers() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    assert_eq!(
        repo["numbers"]["match"],
        info.number_regex
    );
    assert_eq!(
        repo["identifiers"]["match"],
        info.identifier_regex
    );
}

#[test]
fn generate_textmate_patterns_in_correct_order() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let patterns = grammar["patterns"].as_array().unwrap();
    let includes: Vec<&str> = patterns
        .iter()
        .filter_map(|p| p["include"].as_str())
        .collect();

    let doc_comments_pos = includes.iter().position(|s| *s == "#doc_comments");
    let comments_pos = includes.iter().position(|s| *s == "#comments");
    let keywords_pos = includes.iter().position(|s| *s == "#keywords");
    let function_calls_pos = includes.iter().position(|s| *s == "#function_calls");
    let identifiers_pos = includes.iter().position(|s| *s == "#identifiers");

    assert!(doc_comments_pos < comments_pos);
    assert!(comments_pos < keywords_pos);
    assert!(keywords_pos < identifiers_pos);
    assert!(function_calls_pos < identifiers_pos);
}

#[test]
fn generate_textmate_includes_function_calls() {
    let info = sample_info();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    assert!(repo.get("function_calls").is_some());
    let match_str = repo["function_calls"]["match"].as_str().unwrap();
    assert!(match_str.contains(info.identifier_regex.as_str()));
    assert!(match_str.contains(r"\("));
    let captures = &repo["function_calls"]["captures"];
    assert_eq!(captures["1"]["name"], "entity.name.function");
}

#[test]
fn generate_textmate_omits_empty_sections() {
    let mut info = sample_info();
    info.keywords = Vec::new();
    info.booleans = Vec::new();
    info.nulls = Vec::new();
    let grammar = generate_textmate(&info);

    let repo = &grammar["repository"];
    assert!(repo.get("keywords").is_none());
    assert!(repo.get("booleans").is_none());
    assert!(repo.get("nulls").is_none());
}

#[test]
fn generate_textmate_escapes_regex_metacharacters() {
    let mut info = sample_info();
    info.operators = vec![".".to_string(), "*".to_string()];
    let grammar = generate_textmate(&info);

    let op_match = grammar["repository"]["operators"]["match"]
        .as_str()
        .unwrap();
    assert!(op_match.contains(r"\."));
    assert!(op_match.contains(r"\*"));
}
