//! End-to-end: a `LintRuleDef` whose check is `Native("handl:t.check")`
//! dispatches into a `.hnl` function through `HandlNativeLint`
//! (ADR-0097 — handl-authored lint natives, mirroring ADR-0094's
//! `HandlNativeRewrite` for transforms).

use handl_language::{Ast, AstNode, AstRef, AstShapeDef, Language, LintCheck, LintRuleDef, Span};
use hnl_engine_lint::{LintEngine, NativeLintRegistry};
use handl_plugin_lib_handl::{HandlModule, HandlNativeLint};
use serde_json::json;

/// A trivial .hnl lint module: `check` reports one hit (with a fix),
/// `decline` reports none.

/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_canonical::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

const T_HNL: &str = r#"
-- t: test lint module
export check :: m ~> [{name: "x", span: {start: 1, end: 3}, fix: {start: 1, end: 3, replacement: "y"}}]
export decline :: m ~> null
"#;

fn test_lang(check_name: &str) -> Language {
    let mut lang = Language::new();
    lang.merge_ast_fragment(AstShapeDef::new("ast.foo", "foo"))
        .unwrap();
    lang.lints
        .merge(LintRuleDef::new(
            "lint.test.rule",
            "test-rule",
            "warning",
            "violation on '{name}'",
            LintCheck::Native(check_name.into()),
        ))
        .unwrap();
    lang
}

fn test_registry() -> NativeLintRegistry {
    let mut reg = NativeLintRegistry::new();
    reg.add_resolver(
        HandlNativeLint::new()
            .with_module(HandlModule::new("t", T_HNL, &["check", "decline"]))
            .resolver(),
    );
    reg
}

/// ast: 0: foo with span [1,3).
fn test_ast() -> (Ast, AstRef) {
    let mut ast = Ast::new();
    let mut node = AstNode::new("foo").with_payload(json!({"children": []}));
    node.span = Some(Span { start: 1, end: 3 });
    ast.push(node);
    (ast, AstRef::new(0))
}

#[test]
fn handl_lint_fires_with_span_and_fix() {
        ensure_lang();
    let lang = test_lang("handl:t.check");
    let (ast, root) = test_ast();
    let diags = LintEngine::new()
        .with_native_registry(test_registry())
        .lint(&lang, &ast, root)
        .unwrap();
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d.rule, "test-rule");
    assert_eq!(d.message, "violation on 'x'");
    assert_eq!(d.span, Some(Span { start: 1, end: 3 }));
    let fix = d.fix.as_ref().expect("hit carries a fix");
    assert_eq!((fix.start, fix.end), (1, 3));
    assert_eq!(fix.replacement, "y");
}

#[test]
fn handl_lint_null_reports_nothing() {
        ensure_lang();
    let lang = test_lang("handl:t.decline");
    let (ast, root) = test_ast();
    let diags = LintEngine::new()
        .with_native_registry(test_registry())
        .lint(&lang, &ast, root)
        .unwrap();
    assert!(diags.is_empty());
}

#[test]
fn handl_lint_unknown_module_errors() {
        ensure_lang();
    let lang = test_lang("handl:missing.f");
    let (ast, root) = test_ast();
    let err = LintEngine::new()
        .with_native_registry(test_registry())
        .lint(&lang, &ast, root)
        .unwrap_err();
    assert!(format!("{err}").contains("no HandlModule registered under 'missing'"));
}

#[test]
fn handl_lint_unknown_function_errors() {
        ensure_lang();
    let lang = test_lang("handl:t.nope");
    let (ast, root) = test_ast();
    let err = LintEngine::new()
        .with_native_registry(test_registry())
        .lint(&lang, &ast, root)
        .unwrap_err();
    assert!(format!("{err}").contains("has no function 'nope'"));
}

#[test]
fn handl_lint_resolver_declines_non_handl_names() {
        ensure_lang();
    // A map-registered native still wins; an unregistered non-`handl:`
    // name is declined by the resolver and errors as unregistered.
    let mut reg = test_registry();
    reg.register("test.always", |_ctx| {
        Ok(vec![hnl_engine_lint::NativeHit::named(
            "map",
            Span { start: 0, end: 0 },
        )])
    });

    let lang = test_lang("test.always");
    let (ast, root) = test_ast();
    let diags = LintEngine::new()
        .with_native_registry(reg.clone())
        .lint(&lang, &ast, root)
        .unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].message, "violation on 'map'");

    let lang = test_lang("other.missing");
    let err = LintEngine::new()
        .with_native_registry(reg)
        .lint(&lang, &ast, root)
        .unwrap_err();
    assert!(format!("{err}").contains("no native registered for 'other.missing'"));
}
