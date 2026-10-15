use std::cell::RefCell;
use std::sync::Arc;

use crate::{
    BasicSemanticTokens, CanonicalTokenClassifier, SemanticTokenLegend, SemanticTokensPlugin,
    SemanticTokensRequest, TokenClassifier,
};
use handl_kit_canonical::{Kit, SpannedToken};
use handl_language::Token;

thread_local! {
    /// Thread-cached canonical kit — the host-side cache shape the
    /// injected pieces mirror (ADR-0128 §0.8; the kit is a dev-dep).
    static KIT: RefCell<Option<Kit>> = const { RefCell::new(None) };
}

fn with_kit<R>(f: impl FnOnce(&Kit) -> R) -> R {
    KIT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(Kit::new().expect("canonical kit should assemble"));
        }
        f(slot.as_ref().expect("kit populated above"))
    })
}

/// Lex `source` with the canonical kit lexer and return the spanned tokens.
fn lex(source: &str) -> Vec<SpannedToken> {
    with_kit(|kit| kit.lex_with_spans(source).expect("lex should succeed"))
}

/// The classifier over the canonical language — what the host builds via
/// `for_language(kit.language())`.
fn classifier() -> CanonicalTokenClassifier {
    with_kit(|kit| CanonicalTokenClassifier::for_language(kit.language()))
}

/// The provider the host wires: classifier + legend + injected spanned
/// lexer (a capture-less closure over the thread-local kit).
fn provider() -> BasicSemanticTokens {
    BasicSemanticTokens::new(
        classifier(),
        SemanticTokenLegend::default(),
        Arc::new(|src: &str| with_kit(|k| k.lex_with_spans(src))),
    )
}

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
    assert!(
        legend
            .token_modifiers
            .contains(&"documentation".to_string())
    );
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
    let classifier = classifier();
    let (ty, mods) = classifier.classify("token.number", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("number").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_string_to_string_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, mods) = classifier.classify("token.string", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("string").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_keyword_to_keyword_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, mods) = classifier.classify("token.keyword.if", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("keyword").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_bool_to_constant_type() {
    // `true`/`false` are no longer lexer literals (`token.bool` was
    // removed): they are ordinary identifiers bound to Bool globals in
    // `lang.globals` (data/globals.yaml). The context-aware classifier
    // resolves those global names to `constant`.
    let legend = SemanticTokenLegend::default();
    let constant = legend.token_type_index("constant").unwrap();
    let (ty, _mods) = classify_first_identifier("true");
    assert_eq!(ty, constant);
    let (ty, _mods) = classify_first_identifier("false");
    assert_eq!(ty, constant);
}

#[test]
fn classifier_maps_null_to_constant_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, _mods) = classifier.classify("token.null", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("constant").unwrap());
}

#[test]
fn classifier_maps_operator_to_operator_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, mods) = classifier.classify("token.plus", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("operator").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_delimiter_to_punctuation_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, _mods) = classifier.classify("token.lparen", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("punctuation").unwrap());
}

#[test]
fn classifier_maps_identifier_to_variable_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, mods) = classifier.classify("token.identifier", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("variable").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_doc_comment_with_documentation_modifier() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, mods) = classifier.classify("token.doc_comment", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("comment").unwrap());
    assert_eq!(mods, legend.modifier_bit("documentation").unwrap());
}

#[test]
fn classifier_maps_comment_without_modifiers() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, mods) = classifier.classify("token.comment", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("comment").unwrap());
    assert_eq!(mods, 0);
}

#[test]
fn classifier_maps_template_to_string_type() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let (ty, _mods) = classifier.classify("token.template", &legend).unwrap();
    assert_eq!(ty, legend.token_type_index("string").unwrap());
}

#[test]
fn classifier_returns_none_for_unknown_kind() {
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    assert!(classifier.classify("token.unknown", &legend).is_none());
}

#[test]
fn classifier_returns_none_for_unknown_type_name() {
    let legend = SemanticTokenLegend {
        token_types: vec!["keyword".to_string()],
        token_modifiers: vec![],
    };
    let classifier = classifier();
    assert!(classifier.classify("token.number", &legend).is_none());
}

#[test]
fn canonical_semantic_tokens_produces_delta_encoded_tokens() {
    let provider = provider();
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
    assert_eq!(
        first.token_type,
        provider.legend().token_type_index("variable").unwrap()
    );
}

#[test]
fn canonical_semantic_tokens_handles_multiline_source() {
    let provider = provider();
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
    let provider = provider();
    let legend = provider.legend();
    assert_eq!(
        legend.token_types,
        SemanticTokenLegend::default().token_types
    );
    assert_eq!(
        legend.token_modifiers,
        SemanticTokenLegend::default().token_modifiers
    );
}

#[test]
fn canonical_semantic_tokens_empty_source_returns_empty() {
    let provider = provider();
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
    let tokens = lex(source);
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    for (i, token) in tokens.iter().enumerate() {
        if token.kind == "token.identifier" {
            return crate::semantic_tokens::ContextClassifier::classify(
                &classifier,
                &tokens,
                i,
                source,
                &legend,
            )
            .unwrap();
        }
    }
    panic!("no identifier token found in source: {source:?}");
}

/// Helper: lex `source` and classify every identifier token, returning a vec
/// of `(identifier_string, token_type_index, modifier_bit_flags)`.
fn classify_all_identifiers(source: &str) -> Vec<(String, u32, u32)> {
    let tokens = lex(source);
    let legend = SemanticTokenLegend::default();
    let classifier = classifier();
    let mut out = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if token.kind == "token.identifier" {
            let (ty, mods) = crate::semantic_tokens::ContextClassifier::classify(
                &classifier,
                &tokens,
                i,
                source,
                &legend,
            )
            .unwrap();
            out.push((token.text.clone(), ty, mods));
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
    // `crypto.sha256(...)` — `crypto` is the module (variable), `sha256` is the method.
    let all = classify_all_identifiers("crypto.sha256(\"hello\")");
    let legend = SemanticTokenLegend::default();
    let var_idx = legend.token_type_index("variable").unwrap();
    let method_idx = legend.token_type_index("method").unwrap();

    // First identifier: `crypto` → variable (module/namespace)
    let crypto_tok = all.iter().find(|(name, _, _)| name == "crypto").unwrap();
    assert_eq!(crypto_tok.1, var_idx);

    // Second identifier: `sha256` → method (called via dot)
    let sha256_tok = all.iter().find(|(name, _, _)| name == "sha256").unwrap();
    assert_eq!(sha256_tok.1, method_idx);
}

#[test]
fn context_classifier_identifies_type_reference() {
    // "x: MyType.field = 1" — x is a binding declaration, `MyType` is a
    // type ref (PascalCase, followed by `.` for the dotted type namespace).
    let all = classify_all_identifiers("x: MyType.field = 1");
    let legend = SemanticTokenLegend::default();
    let type_idx = legend.token_type_index("type").unwrap();

    // Find the `MyType` identifier (the lexer splits `MyType.field` into
    // separate `MyType`, `.`, `field` tokens).
    let type_tok = all
        .iter()
        .find(|(s, _, _)| s == "MyType")
        .unwrap_or_else(|| panic!("expected `MyType` identifier, got {all:?}"));
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
fn context_classifier_identifies_binding_declaration_with_bind() {
    let (ty, mods) = classify_first_identifier("x :: 42");
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
    // make_flag :: (name: type.string, short: type.string): Flag ~>
    //   { name: name, short: short };
    let source = "make_flag :: (name: type.string, short: type.string): Flag ~>\n  { name: name, short: short };";
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

// ---------------------------------------------------------------------------
// Token construction helpers (verify Token struct shape)
// ---------------------------------------------------------------------------

#[test]
fn token_struct_has_kind_and_text() {
    let t = Token {
        kind: "token.identifier".into(),
        text: "foo".into(),
        ..Default::default()
    };
    assert_eq!(t.kind, "token.identifier");
    assert_eq!(t.text, "foo");
}
