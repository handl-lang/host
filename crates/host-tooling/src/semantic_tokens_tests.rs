use crate::{
    CanonicalTokenClassifier, SemanticTokenLegend, SemanticTokensPlugin, SemanticTokensRequest,
    TokenClassifier, canonical_semantic_tokens,
};
use handl_kit_canonical::{LexDelimiter, LexKind, LexKeyword, LexOperator, LexTemplateSegment};
use handl_plugin_lexer::Lexer;

#[test]
fn legend_default_has_standard_token_types() {
    let legend = SemanticTokenLegend::default();
    assert!(legend.token_types.contains(&"keyword".to_string()));
    assert!(legend.token_types.contains(&"number".to_string()));
    assert!(legend.token_types.contains(&"string".to_string()));
    assert!(legend.token_types.contains(&"comment".to_string()));
    assert!(legend.token_types.contains(&"operator".to_string()));
    assert!(legend.token_types.contains(&"punctuation".to_string()));
    assert!(legend.token_types.contains(&"variable".to_string()));
}

#[test]
fn legend_default_has_standard_modifiers() {
    let legend = SemanticTokenLegend::default();
    assert!(legend.token_modifiers.contains(&"declaration".to_string()));
    assert!(legend
        .token_modifiers
        .contains(&"documentation".to_string()));
}

#[test]
fn legend_token_type_index_returns_correct_position() {
    let legend = SemanticTokenLegend::default();
    assert_eq!(legend.token_type_index("keyword"), Some(0));
    assert_eq!(legend.token_type_index("number"), Some(1));
    assert_eq!(legend.token_type_index("string"), Some(2));
    assert_eq!(legend.token_type_index("nonexistent"), None);
}

#[test]
fn legend_modifier_bit_returns_correct_bit_flag() {
    let legend = SemanticTokenLegend::default();
    assert_eq!(legend.modifier_bit("declaration"), Some(1));
    assert_eq!(legend.modifier_bit("documentation"), Some(2));
    assert_eq!(legend.modifier_bit("nonexistent"), None);
}

#[test]
fn classifier_maps_number_to_number_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(&LexKind::Number(42.0), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("number").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_string_to_string_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(&LexKind::String("hello".to_string()), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("string").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_keyword_to_keyword_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(
            &LexKind::Keyword(LexKeyword::Match),
            &legend,
        )
        .unwrap();
    assert_eq!(ty, legend.token_type_index("keyword").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_bool_to_keyword_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, _mods) = classifier
        .classify(&LexKind::Bool(true), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("keyword").unwrap());
}

#[test]
fn classifier_maps_null_to_keyword_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, _mods) = classifier
        .classify(&LexKind::Null, &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("keyword").unwrap());
}

#[test]
fn classifier_maps_operator_to_operator_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(
            &LexKind::Operator(LexOperator::Plus),
            &legend,
        )
        .unwrap();
    assert_eq!(ty, legend.token_type_index("operator").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_delimiter_to_punctuation_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, _mods) = classifier
        .classify(
            &LexKind::Delimiter(LexDelimiter::ParenOpen),
            &legend,
        )
        .unwrap();
    assert_eq!(ty, legend.token_type_index("punctuation").unwrap());
}

#[test]
fn classifier_maps_identifier_to_variable_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(&LexKind::Identifier("x".to_string()), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("variable").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_doc_comment_with_documentation_modifier() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(
            &LexKind::DocComment("/// doc".to_string()),
            &legend,
        )
        .unwrap();
    assert_eq!(ty, legend.token_type_index("comment").unwrap());
    assert_eq!(mods, legend.modifier_bit("documentation").unwrap());
}

#[test]
fn classifier_maps_comment_without_modifiers() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(&LexKind::Comment("// comment".to_string()), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("comment").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_template_to_string_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, _mods) = classifier
        .classify(&LexKind::Template(vec![LexTemplateSegment::Text("hello".into())]), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("string").unwrap());
}

#[test]
fn classifier_returns_none_for_eof() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    assert!(classifier.classify(&LexKind::Eof, &legend).is_none());
}

#[test]
fn classifier_returns_none_for_unknown_type_name() {
    let legend = SemanticTokenLegend {
        token_types: vec!["keyword".to_string()],
        token_modifiers: vec![],
    };
    let classifier = CanonicalTokenClassifier;
    assert!(classifier
        .classify(&LexKind::Number(42.0), &legend)
        .is_none());
}

#[test]
fn canonical_semantic_tokens_produces_delta_encoded_tokens() {
    let provider = canonical_semantic_tokens();
    let resp = provider
        .run(&SemanticTokensRequest {
            source: "x = 42".to_string(),
        })
        .unwrap();

    assert!(!resp.0.is_empty());

    let first = &resp.0[0];
    assert_eq!(first.delta_line, 0);
    assert_eq!(first.delta_start, 0);
    assert_eq!(first.length, 1);
    // "x" is lexed as an identifier (variable).
    assert_eq!(first.token_type, provider.legend().token_type_index("variable").unwrap());
}

#[test]
fn canonical_semantic_tokens_handles_multiline_source() {
    let provider = canonical_semantic_tokens();
    let resp = provider
        .run(&SemanticTokensRequest {
            source: "x = 1\ny = 2".to_string(),
        })
        .unwrap();

    assert!(resp.0.len() >= 6);

    // First "x" on line 0
    let x1 = &resp.0[0];
    assert_eq!(x1.delta_line, 0);
    assert_eq!(x1.delta_start, 0);

    // Second "y" on line 1 — tokens: x, =, 1, y, =, 2
    // Index 3 is "y"
    let y1 = &resp.0[3];
    assert_eq!(y1.delta_line, 1);
    assert_eq!(y1.delta_start, 0);
}

#[test]
fn canonical_semantic_tokens_legend_matches_default() {
    let provider = canonical_semantic_tokens();
    let legend = provider.legend();
    assert_eq!(legend.token_types, SemanticTokenLegend::default().token_types);
    assert_eq!(
        legend.token_modifiers,
        SemanticTokenLegend::default().token_modifiers
    );
}

#[test]
fn canonical_semantic_tokens_empty_source_returns_empty() {
    let provider = canonical_semantic_tokens();
    let resp = provider
        .run(&SemanticTokensRequest {
            source: "".to_string(),
        })
        .unwrap();
    assert!(resp.0.is_empty());
}

// ---------------------------------------------------------------------------
// Context-aware classifier tests
// ---------------------------------------------------------------------------

/// Helper: lex `source` with the canonical lexer and classify the first
/// identifier token using the context-aware classifier, returning
/// `(token_type_index, modifier_bit_flags)`.
fn classify_first_identifier(source: &str) -> (u32, u32) {
    use handl_kit_canonical::LexLexer;
    let lexer = LexLexer;
    let tokens: Vec<handl_plugin_lexer::Token<LexKind>> =
        lexer.lex(source).unwrap();
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    for (i, token) in tokens.iter().enumerate() {
        if matches!(token.kind, LexKind::Identifier(_)) {
            return crate::semantic_tokens::ContextClassifier::classify(
                &classifier, &tokens, i, source, &legend,
            )
            .unwrap();
        }
    }
    panic!("no identifier token found in source: {source:?}");
}

/// Helper: lex `source` and classify every identifier token, returning a vec
/// of `(identifier_string, token_type_index, modifier_bit_flags)`.
fn classify_all_identifiers(
    source: &str,
) -> Vec<(String, u32, u32)> {
    use handl_kit_canonical::LexLexer;
    let lexer = LexLexer;
    let tokens: Vec<handl_plugin_lexer::Token<LexKind>> =
        lexer.lex(source).unwrap();
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let mut out = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if let LexKind::Identifier(s) = &token.kind {
            let (ty, mods) = crate::semantic_tokens::ContextClassifier::classify(
                &classifier, &tokens, i, source, &legend,
            )
            .unwrap();
            out.push((s.clone(), ty, mods));
        }
    }
    out
}

#[test]
fn legend_default_has_context_aware_token_types() {
    let legend = SemanticTokenLegend::default();
    assert!(legend.token_types.contains(&"type".to_string()));
    assert!(legend.token_types.contains(&"parameter".to_string()));
    assert!(legend.token_types.contains(&"property".to_string()));
    assert!(legend.token_types.contains(&"function".to_string()));
}

#[test]
fn context_classifier_identifies_function_call() {
    let (ty, mods) = classify_first_identifier("foo(x)");
    let legend = SemanticTokenLegend::default();
    assert_eq!(ty, legend.token_type_index("function").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn context_classifier_identifies_dotted_function_call() {
    let (ty, _mods) = classify_first_identifier("crypto.sha256(\"hello\")");
    let legend = SemanticTokenLegend::default();
    assert_eq!(ty, legend.token_type_index("function").unwrap());
}

#[test]
fn context_classifier_identifies_type_reference() {
    // "x: type.str = 1" — x is a binding declaration, type.str is a type ref.
    let all = classify_all_identifiers("x: type.str = 1");
    let legend = SemanticTokenLegend::default();
    let type_idx = legend.token_type_index("type").unwrap();

    // Find the type.str identifier.
    let type_tok = all
        .iter()
        .find(|(s, _, _)| s == "type.str")
        .unwrap_or_else(|| panic!("expected type.str identifier, got {all:?}"));
    assert_eq!(type_tok.1, type_idx);
}

#[test]
fn context_classifier_identifies_pascalcase_type() {
    let all = classify_all_identifiers("x: Flag = 1");
    let legend = SemanticTokenLegend::default();
    let type_idx = legend.token_type_index("type").unwrap();
    let flag = all
        .iter()
        .find(|(s, _, _)| s == "Flag")
        .unwrap_or_else(|| panic!("expected Flag identifier, got {all:?}"));
    assert_eq!(flag.1, type_idx);
}

#[test]
fn context_classifier_identifies_binding_declaration_with_colon() {
    let (ty, mods) = classify_first_identifier("hash: type.str = 1");
    let legend = SemanticTokenLegend::default();
    assert_eq!(ty, legend.token_type_index("variable").unwrap());
    assert_eq!(mods, legend.modifier_bit("declaration").unwrap());
}

#[test]
fn context_classifier_identifies_binding_declaration_with_equal() {
    let (ty, mods) = classify_first_identifier("x = 42");
    let legend = SemanticTokenLegend::default();
    assert_eq!(ty, legend.token_type_index("variable").unwrap());
    assert_eq!(mods, legend.modifier_bit("declaration").unwrap());
}

#[test]
fn context_classifier_identifies_parameter() {
    let all = classify_all_identifiers("(name: type.string, short: type.string)");
    let legend = SemanticTokenLegend::default();
    let param_idx = legend.token_type_index("parameter").unwrap();

    let name_tok = all
        .iter()
        .find(|(s, _, _)| s == "name")
        .unwrap_or_else(|| panic!("expected name identifier, got {all:?}"));
    assert_eq!(name_tok.1, param_idx);

    let short_tok = all
        .iter()
        .find(|(s, _, _)| s == "short")
        .unwrap_or_else(|| panic!("expected short identifier, got {all:?}"));
    assert_eq!(short_tok.1, param_idx);
}

#[test]
fn context_classifier_identifies_property_name() {
    let source = "{ name: type.string, short: type.string }";
    let all = classify_all_identifiers(source);
    let legend = SemanticTokenLegend::default();
    let prop_idx = legend.token_type_index("property").unwrap();

    let name_tok = all
        .iter()
        .find(|(s, _, _)| s == "name")
        .unwrap_or_else(|| panic!("expected name identifier, got {all:?}"));
    assert_eq!(name_tok.1, prop_idx);

    let short_tok = all
        .iter()
        .find(|(s, _, _)| s == "short")
        .unwrap_or_else(|| panic!("expected short identifier, got {all:?}"));
    assert_eq!(short_tok.1, prop_idx);
}

#[test]
fn context_classifier_default_identifier_is_variable() {
    let all = classify_all_identifiers("x + y");
    let legend = SemanticTokenLegend::default();
    let var_idx = legend.token_type_index("variable").unwrap();

    for (s, ty, mods) in &all {
        assert_eq!(*ty, var_idx, "identifier {s:?} should be variable");
        assert_eq!(*mods, 0, "identifier {s:?} should have no modifiers");
    }
}

#[test]
fn context_classifier_function_definition_binding() {
    // make_flag = (name: type.string, short: type.string): Flag ->
    //   { name: name, short: short };
    let source = "make_flag = (name: type.string, short: type.string): Flag ->\n  { name: name, short: short };";
    let all = classify_all_identifiers(source);
    let legend = SemanticTokenLegend::default();
    let var_idx = legend.token_type_index("variable").unwrap();
    let decl_mod = legend.modifier_bit("declaration").unwrap();
    let param_idx = legend.token_type_index("parameter").unwrap();
    let type_idx = legend.token_type_index("type").unwrap();
    let prop_idx = legend.token_type_index("property").unwrap();

    // make_flag: binding declaration (variable + declaration)
    let make_flag = all.iter().find(|(s, _, _)| s == "make_flag").unwrap();
    assert_eq!(make_flag.1, var_idx);
    assert_eq!(make_flag.2, decl_mod);

    // name (param): parameter — verify at least one "name" is a parameter
    let name_param = all
        .iter()
        .find(|(s, ty, _)| s == "name" && *ty == param_idx)
        .unwrap_or_else(|| panic!("expected a parameter 'name', got {all:?}"));
    let _ = name_param;

    // Flag: type
    let flag = all.iter().find(|(s, _, _)| s == "Flag").unwrap();
    assert_eq!(flag.1, type_idx);

    // Count parameter vs property occurrences of "name" and "short".
    let name_params = all
        .iter()
        .filter(|(s, ty, _)| s == "name" && *ty == param_idx)
        .count();
    let name_props = all
        .iter()
        .filter(|(s, ty, _)| s == "name" && *ty == prop_idx)
        .count();
    assert_eq!(name_params, 1, "expected one parameter 'name', got {all:?}");
    assert_eq!(name_props, 1, "expected one property 'name', got {all:?}");
}
