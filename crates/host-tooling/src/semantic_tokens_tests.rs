use crate::{
    CanonicalTokenClassifier, SemanticTokenLegend, SemanticTokensPlugin, SemanticTokensRequest,
    TokenClassifier, canonical_semantic_tokens,
};
use handl_kit_canonical::{CanonicalDelim, CanonicalKind, CanonicalKeyword, CanonicalOp, CanonicalTemplateSegment};

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
        .classify(&CanonicalKind::Number(42.0), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("number").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_string_to_string_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(&CanonicalKind::String("hello".to_string()), &legend)
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
            &CanonicalKind::Keyword(CanonicalKeyword::Match),
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
        .classify(&CanonicalKind::Bool(true), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("keyword").unwrap());
}

#[test]
fn classifier_maps_null_to_keyword_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, _mods) = classifier
        .classify(&CanonicalKind::Null, &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("keyword").unwrap());
}

#[test]
fn classifier_maps_operator_to_operator_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, mods) = classifier
        .classify(
            &CanonicalKind::Operator(CanonicalOp::Plus),
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
            &CanonicalKind::Delimiter(CanonicalDelim::ParenOpen),
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
        .classify(&CanonicalKind::Identifier("x".to_string()), &legend)
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
            &CanonicalKind::DocComment("/// doc".to_string()),
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
        .classify(&CanonicalKind::Comment("// comment".to_string()), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("comment").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_template_to_string_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    let (ty, _mods) = classifier
        .classify(&CanonicalKind::Template(vec![CanonicalTemplateSegment::Text("hello".into())]), &legend)
        .unwrap();
    assert_eq!(ty, legend.token_type_index("string").unwrap());
}

#[test]
fn classifier_returns_none_for_eof() {
    let legend = SemanticTokenLegend::default();
    let classifier = CanonicalTokenClassifier;
    assert!(classifier.classify(&CanonicalKind::Eof, &legend).is_none());
}

#[test]
fn classifier_returns_none_for_unknown_type_name() {
    let legend = SemanticTokenLegend {
        token_types: vec!["keyword".to_string()],
        token_modifiers: vec![],
    };
    let classifier = CanonicalTokenClassifier;
    assert!(classifier
        .classify(&CanonicalKind::Number(42.0), &legend)
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
