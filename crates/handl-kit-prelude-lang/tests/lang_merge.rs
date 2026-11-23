//! Eval-path language assembly tests for `handl-kit-prelude-lang`
//! (moved from `handl-kit-data-prelude` when the crate split).

use handl_kit_prelude_lang::{language_from_yaml, merge_lang_fragments};

#[test]
fn language_from_yaml_produces_language() {
    let lang = language_from_yaml().unwrap();
    assert!(lang.tokens.len() > 0);
    assert!(lang.grammar.len() > 0);
    assert!(lang.eval.len() > 0);
}

#[test]
fn types_yaml_loads() {
    let lang = language_from_yaml().unwrap();
    // +13 Term frontier type rules + 1 transport rule + 1 axis_flow rule
    // (ADR-0120 Phase 2) + `type.family`/`type.data`/`type.loop`
    // (ADR-0120 Phase 4 dependent structure).
    // +3 `type.cell_id`/`type.cell_comp_v`/`type.cell_comp_h` (ADR-0120
    // globular cell algebra) + `type.index_app` (phase-2 WS-C) +
    // `type.dimension` (the `<>` index bracket — same synth_call scheme
    // as `type.index_app`, post `{`→`<` migration) +
    // `type.instance` (phase-3 WS-A instance declarations) +
    // `type.exists` (the `exists m : S, T` `Sigma`-shorthand keyword —
    // `forall`'s existential twin).
    assert_eq!(lang.types.len(), 89);
    // The `value` kind (literals) must have a rule.
    assert!(lang.types.get_by_identity("value").is_some());
    // Spot-check a few meaningful rules.
    assert!(lang.types.get_by_identity("var").is_some());
    assert!(lang.types.get_by_identity("if").is_some());
    assert!(lang.types.get_by_identity("binary.add").is_some());
    assert!(lang.types.get_by_identity("unary.neg").is_some());
    assert!(lang.types.get_by_identity("paren").is_some());
}

#[test]
fn transforms_yaml_loads_first_class_rules() {
    // ADR-0094: transform rules are authored in `data/transforms.yaml`
    // and land in `lang.transforms` directly — identical semantics to
    // the old `desugar:`-field lowering (same ids, pass, conditions,
    // and rewrite steps).
    use handl_language::{RewriteStep, TransformCondition, TransformRule};
    let lang = language_from_yaml().unwrap();
    // 2 baseline rules (entry, lower.entry) + 6 desugar rules
    // (do_while, nullish, opt-chain, while, while.state, for)
    // + 1 `desugar.dimension` (ADR-0106 §11/§12: `f<X>` lowers to the
    // application `f(X)` by re-tagging the node as a `call`).
    // + 1 `meta-transform` placeholder for N-dimensional arrows.
    // + 1 `n_dim_opt` flow rule.
    // +1 exec-erasure pass — type-annot erases to subject (ADR-0154 §6).
    // +1 `desugar.exists` — `exists m : S, T` lowers to `Sigma(S, m ~> T)`
    // on the exec path.
    assert_eq!(lang.transforms.len(), 13);
    assert_eq!(
        lang.transforms_for("desugar", "do-while")[0].id,
        "transform.desugar.do_while"
    );

    let dimension = &lang.transforms_for("desugar", "dimension")[0];
    assert_eq!(dimension.id, "transform.desugar.dimension");
    assert_eq!(dimension.priority, 0);
    assert_eq!(
        dimension.rule,
        TransformRule::Declarative {
            target_kind: "call".into(),
            children: vec![RewriteStep::Keep(0), RewriteStep::KeepRest(1)],
            payload: None,
        }
    );

    let entry = &lang.transforms_for("desugar", "entry")[0];
    assert_eq!(entry.id, "transform.desugar.entry");
    assert_eq!(
        entry.when,
        Some(TransformCondition::KindAt {
            index: Some(0),
            role: None,
            kind: "array".into()
        })
    );
    assert_eq!(
        entry.rule,
        TransformRule::Declarative {
            target_kind: "entry".into(),
            children: vec![
                RewriteStep::Create {
                    kind: "paren".into(),
                    payload: None,
                    children: vec![RewriteStep::KeepPath(vec![0, 0])],
                },
                RewriteStep::Keep(1),
            ],
            payload: None,
        }
    );
}

#[test]
fn desugar_field_still_lowers_into_transforms() {
    // Backward compat (ADR-0094): an `AstShapeDef` carrying a `desugar:`
    // rule still lowers into a `TransformDef` in pass "desugar" — the
    // field is kept for third-party fragments.
    let ast_yaml = r#"
- id: ast.sugared
  node_kind: sugared
  children:
    - role: a
    - role: b
  desugar:
    target_kind: plain
    when:
      child_index: 0
      child_kind: paren
    children:
      - !Keep 0
      - !Keep 1
"#;
    let lang = merge_lang_fragments(
        "[]", "[]", "[]", "[]", ast_yaml, "[]", "[]", "[]", "[]",
    )
    .unwrap();
    let rules = lang.transforms_for("desugar", "sugared");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].id, "transform.desugar.sugared");
    assert_eq!(rules[0].pass, "desugar");
}

#[test]
fn canonical_transforms_validate_clean() {
    // ADR-0094 "Production readiness": `validate_transforms` runs at
    // `Language` build (see `merge_lang_fragments`); the canonical kit must
    // produce zero Error-severity diagnostics.
    let lang = language_from_yaml().unwrap();
    let diags = hnl_engine_transform::validate::validate_transforms(&lang);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == handl_plugin_ext_diag::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected transform errors: {errors:?}");
}

#[test]
fn merge_rejects_invalid_transform_rules() {
    // A `desugar:` rule whose `Keep` index is out of bounds for the
    // shape's declared roles must fail `Language` assembly, not pass
    // silently (ADR-0094: no silent fallthrough).
    let ast_yaml = r#"
- id: ast.bogus
  node_kind: bogus
  children:
    - role: only
  desugar:
    target_kind: bogus
    children:
      - !Keep 7
"#;
    let err = merge_lang_fragments(
        "[]", "[]", "[]", "[]", ast_yaml, "[]", "[]", "[]", "[]",
    )
    .unwrap_err();
    assert!(
        format!("{err}").contains("transform rule validation failed"),
        "expected validation failure, got: {err}"
    );
}

#[test]
fn excluded_fragments_absent() {
    let lang = language_from_yaml().unwrap();
    for id in handl_kit_prelude_lang::EXCLUDED_FRAGMENT_IDS {
        if id.starts_with("eval.") {
            assert!(lang.eval.get_by_id("eval.call").is_some());
        } else if id.starts_with("grammar.") {
            assert!(lang.grammar.get_by_id("grammar.call.infix").is_some());
        }
    }
}
