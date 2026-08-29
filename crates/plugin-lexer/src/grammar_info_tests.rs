use crate::{GrammarInfo, GrammarInfoProvider, KitGrammarMetadata};

#[test]
fn grammar_info_is_serializable() {
    let info = GrammarInfo {
        scope_name: "source.test".to_string(),
        file_extensions: vec!["tst".to_string()],
        keywords: vec!["let".to_string()],
        booleans: vec!["true".to_string()],
        nulls: vec!["null".to_string()],
        operators: vec!["+".to_string()],
        delimiters: vec!["(".to_string()],
        line_comment_start: Some("//".to_string()),
        block_comment: Some(("/*".to_string(), "*/".to_string())),
        doc_line_comment_start: None,
        doc_block_comment: None,
        string_delimiters: vec!['"'],
        number_regex: r"\d+".to_string(),
        identifier_regex: r"[a-z]+".to_string(),
    };

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: GrammarInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.scope_name, "source.test");
    assert_eq!(deserialized.keywords, vec!["let".to_string()]);
    assert_eq!(deserialized.line_comment_start, Some("//".to_string()));
    assert_eq!(deserialized.doc_line_comment_start, None);
}

#[test]
fn grammar_info_with_no_comments() {
    let info = GrammarInfo {
        scope_name: "source.plain".to_string(),
        file_extensions: vec!["plain".to_string()],
        keywords: Vec::new(),
        booleans: Vec::new(),
        nulls: Vec::new(),
        operators: Vec::new(),
        delimiters: Vec::new(),
        line_comment_start: None,
        block_comment: None,
        doc_line_comment_start: None,
        doc_block_comment: None,
        string_delimiters: Vec::new(),
        number_regex: r"\d+".to_string(),
        identifier_regex: r"[a-z]+".to_string(),
    };

    assert!(info.line_comment_start.is_none());
    assert!(info.block_comment.is_none());
    assert!(info.doc_line_comment_start.is_none());
    assert!(info.doc_block_comment.is_none());
}

#[test]
fn grammar_info_provider_trait_can_be_generic() {
    struct DummyKit;
    impl GrammarInfoProvider for DummyKit {
        fn grammar_info(&self) -> GrammarInfo {
            GrammarInfo {
                scope_name: "source.dummy".to_string(),
                file_extensions: vec!["dum".to_string()],
                keywords: Vec::new(),
                booleans: Vec::new(),
                nulls: Vec::new(),
                operators: Vec::new(),
                delimiters: Vec::new(),
                line_comment_start: None,
                block_comment: None,
                doc_line_comment_start: None,
                doc_block_comment: None,
                string_delimiters: Vec::new(),
                number_regex: r"\d+".to_string(),
                identifier_regex: r"[a-z]+".to_string(),
            }
        }
    }

    let kit = DummyKit;
    let info = kit.grammar_info();
    assert_eq!(info.scope_name, "source.dummy");
}

#[test]
fn kit_grammar_metadata_trait_can_be_implemented() {
    struct DummyKit;
    impl KitGrammarMetadata for DummyKit {
        fn scope_name(&self) -> &str {
            "source.dummy"
        }
        fn file_extensions(&self) -> Vec<String> {
            vec!["dum".to_string()]
        }
    }

    let kit = DummyKit;
    assert_eq!(kit.scope_name(), "source.dummy");
    assert_eq!(kit.file_extensions(), vec!["dum".to_string()]);
}

#[test]
fn kit_grammar_metadata_decouples_scope_from_grammar_info() {
    struct DummyKit;
    impl KitGrammarMetadata for DummyKit {
        fn scope_name(&self) -> &str {
            "source.custom"
        }
        fn file_extensions(&self) -> Vec<String> {
            vec!["custom".to_string(), "cst".to_string()]
        }
    }
    impl GrammarInfoProvider for DummyKit {
        fn grammar_info(&self) -> GrammarInfo {
            GrammarInfo {
                scope_name: self.scope_name().to_string(),
                file_extensions: self.file_extensions(),
                keywords: Vec::new(),
                booleans: Vec::new(),
                nulls: Vec::new(),
                operators: Vec::new(),
                delimiters: Vec::new(),
                line_comment_start: None,
                block_comment: None,
                doc_line_comment_start: None,
                doc_block_comment: None,
                string_delimiters: Vec::new(),
                number_regex: r"\d+".to_string(),
                identifier_regex: r"[a-z]+".to_string(),
            }
        }
    }

    let kit = DummyKit;
    let info = kit.grammar_info();
    assert_eq!(info.scope_name, "source.custom");
    assert_eq!(info.file_extensions, vec!["custom".to_string(), "cst".to_string()]);
}
