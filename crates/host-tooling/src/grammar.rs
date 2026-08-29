use serde_json::{Map, Value, json};

pub use handl_plugin_lexer::{GrammarInfo, GrammarInfoProvider, KitGrammarMetadata};

/// Generate a TextMate grammar JSON from a [`GrammarInfo`].
///
/// The output is a `.tmLanguage.json`-compatible JSON value that can be
/// consumed by VS Code, Sublime Text, TextMate, GitHub Linguist, etc.
pub fn generate_textmate(info: &GrammarInfo) -> Value {
    let mut repository = Map::new();

    // Comments
    if let Some(ref prefix) = info.line_comment_start {
        let escaped = regex_escape(prefix);
        repository.insert(
            "comments".into(),
            json!({
                "name": "comment.line",
                "match": format!("{escaped}.*")
            }),
        );
    }
    if let Some((start, end)) = &info.block_comment {
        let s = regex_escape(start);
        let e = regex_escape(end);
        repository.insert(
            "block_comment".into(),
            json!({
                "name": "comment.block",
                "begin": s,
                "end": e,
                "patterns": []
            }),
        );
    }
    if let Some(ref prefix) = info.doc_line_comment_start {
        let escaped = regex_escape(prefix);
        repository.insert(
            "doc_comments".into(),
            json!({
                "name": "comment.documentation",
                "match": format!("{escaped}.*")
            }),
        );
    }
    if let Some((start, end)) = &info.doc_block_comment {
        let s = regex_escape(start);
        let e = regex_escape(end);
        repository.insert(
            "doc_block_comment".into(),
            json!({
                "name": "comment.documentation.block",
                "begin": s,
                "end": e,
                "patterns": []
            }),
        );
    }

    // Strings
    if !info.string_delimiters.is_empty() {
        let string_patterns: Vec<Value> = info
            .string_delimiters
            .iter()
            .map(|&delim| {
                let d = regex_escape(&delim.to_string());
                json!({
                    "name": "string.quoted",
                    "begin": d,
                    "end": d,
                    "patterns": []
                })
            })
            .collect();
        repository.insert("strings".into(), json!({ "patterns": string_patterns }));
    }

    // Numbers
    repository.insert(
        "numbers".into(),
        json!({
            "name": "constant.numeric",
            "match": info.number_regex
        }),
    );

    // Keywords
    if !info.keywords.is_empty() {
        let alternation = info
            .keywords
            .iter()
            .map(|k| regex_escape(k))
            .collect::<Vec<_>>()
            .join("|");
        repository.insert(
            "keywords".into(),
            json!({
                "name": "keyword",
                "match": format!(r"\b({alternation})\b")
            }),
        );
    }

    // Booleans
    if !info.booleans.is_empty() {
        let alternation = info
            .booleans
            .iter()
            .map(|b| regex_escape(b))
            .collect::<Vec<_>>()
            .join("|");
        repository.insert(
            "booleans".into(),
            json!({
                "name": "constant.language.boolean",
                "match": format!(r"\b({alternation})\b")
            }),
        );
    }

    // Null
    if !info.nulls.is_empty() {
        let alternation = info
            .nulls
            .iter()
            .map(|n| regex_escape(n))
            .collect::<Vec<_>>()
            .join("|");
        repository.insert(
            "nulls".into(),
            json!({
                "name": "constant.language.null",
                "match": format!(r"\b({alternation})\b")
            }),
        );
    }

    // Operators
    if !info.operators.is_empty() {
        let alternation = info
            .operators
            .iter()
            .map(|o| regex_escape(o))
            .collect::<Vec<_>>()
            .join("|");
        repository.insert(
            "operators".into(),
            json!({
                "name": "keyword.operator",
                "match": format!("({alternation})")
            }),
        );
    }

    // Delimiters
    if !info.delimiters.is_empty() {
        let alternation = info
            .delimiters
            .iter()
            .map(|d| regex_escape(d))
            .collect::<Vec<_>>()
            .join("|");
        repository.insert(
            "delimiters".into(),
            json!({
                "name": "punctuation",
                "match": format!("[{alternation}]")
            }),
        );
    }

    // Identifiers (must come after keywords so they don't shadow)
    repository.insert(
        "identifiers".into(),
        json!({
            "name": "variable",
            "match": info.identifier_regex
        }),
    );

    // Build top-level patterns (order matters for TextMate)
    let mut patterns: Vec<Value> = Vec::new();

    if info.doc_line_comment_start.is_some() {
        patterns.push(json!({ "include": "#doc_comments" }));
    }
    if info.doc_block_comment.is_some() {
        patterns.push(json!({ "include": "#doc_block_comment" }));
    }
    if info.line_comment_start.is_some() {
        patterns.push(json!({ "include": "#comments" }));
    }
    if info.block_comment.is_some() {
        patterns.push(json!({ "include": "#block_comment" }));
    }
    patterns.push(json!({ "include": "#strings" }));
    patterns.push(json!({ "include": "#numbers" }));
    if !info.booleans.is_empty() {
        patterns.push(json!({ "include": "#booleans" }));
    }
    if !info.nulls.is_empty() {
        patterns.push(json!({ "include": "#nulls" }));
    }
    if !info.keywords.is_empty() {
        patterns.push(json!({ "include": "#keywords" }));
    }
    if !info.operators.is_empty() {
        patterns.push(json!({ "include": "#operators" }));
    }
    if !info.delimiters.is_empty() {
        patterns.push(json!({ "include": "#delimiters" }));
    }
    patterns.push(json!({ "include": "#identifiers" }));

    json!({
        "scopeName": info.scope_name,
        "patterns": patterns,
        "repository": repository,
    })
}

fn regex_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$'
            | '/' => {
                format!("\\{c}")
            }
            _ => c.to_string(),
        })
        .collect()
}
