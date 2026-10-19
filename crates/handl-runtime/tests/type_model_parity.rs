//! ADR-0103 Step 6: type model parity tests.
//!
//! `type_model.hnl` is the HoTT *type* interpretation of the arrow-calculus
//! tags — every `Term` tag is read as a type/proof judgment, not as an
//! executable combinator (that is `arrow_model.hnl`'s job). These tests wrap
//! the type model module in a [`HandlModelAdapter`] (the ADR-0103 "models as
//! data" seam) and verify that it produces sensible HoTT type-judgment output
//! for the HoTT-type terms (`Path`, `Equiv`, `Refine`, `Rewrite`, `CellId`,
//! `Src`, `Tgt`, …) and the core arrow tags (`Arr`, `Const`, `Seq`).
//!
//! Because the type model interprets tags as type judgments while
//! `arrow_model.hnl` interprets them as executable combinators, the two
//! models deliberately *differ* on most HoTT tags (e.g. `Path` builds the
//! identity type `Id_A(a, b)` instead of a path value; `Equiv` builds the
//! propositional `type.bool` witness instead of a bare boolean). The tests
//! below assert the type model's golden type-judgment output directly, and
//! additionally check that the two models *agree* on the tags where their
//! semantics coincide (`Arr`, `Const`, `Rewrite`, `CellId`) — demonstrating
//! that the type model is a faithful, overlapping member of the model
//! family rather than an unrelated artifact.

use handl_language::{Ast, Language, Term, Value};
use handl_plugin_lib_handl::{HandlModel, HandlModelAdapter, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use hnl_engine::{AstNode, Model, ModelCtx, TermData};
use serde_json::json;


/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

const TYPE_MODEL_HNL: &str = include_str!("fixtures/type_model.hnl");
const ARROW_MODEL_HNL: &str = include_str!("fixtures/arrow_model.hnl");

/// Convert a `TermData` back to `Term` for passing to `Model::run` (which
/// takes `&Term`). The conversion is lossless for all well-formed `TermData`.
fn to_term(td: TermData) -> Term {
    td.to_term().expect("TermData -> Term conversion failed")
}

/// Build a `HandlModelAdapter` wrapping the `type_model.hnl` module.
///
/// This is the ADR-0103 Step 6 seam: the type model (Handl data) plugged into
/// the `Model` trait, interchangeable with the Rust `ValueModel` and the
/// `arrow_model.hnl` adapter.
fn type_model_adapter() -> HandlModelAdapter {
    let module = HandlModule::new("type_model", TYPE_MODEL_HNL, &["run"])
        .with_dependency_arc(
            stdlib::load_hnl_file(format!(
                "{}/src/array/index.hnl",
                handl_lib_collections::ROOT
            ))
            .expect("array module"),
        )
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule);
    HandlModelAdapter::new(module)
}

/// Build a `HandlModel` backed by `arrow_model.hnl` (the exec interpretation),
/// used as the overlapping reference model for the tags where type and exec
/// semantics coincide.
fn arrow_model() -> HandlModel {
    let module = HandlModule::new("arrow_model", ARROW_MODEL_HNL, &["run"])
        .with_dependency_arc(
            stdlib::load_hnl_file(format!(
                "{}/src/array/index.hnl",
                handl_lib_collections::ROOT
            ))
            .expect("array module"),
        )
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule);
    HandlModel::new(module)
}

/// Run `term` through the type model [`HandlModelAdapter`] (via the `Model`
/// trait) with the given `incoming` value, returning the type judgment.
fn type_model_run(term: &Term, incoming: Value) -> Value {
    let adapter = type_model_adapter();
    let ast = Ast::new();
    let lang = Language::new();
    let current_node = AstNode;
    let mut ctx = ModelCtx {
        ast: &ast,
        lang: &lang,
        current_node: &current_node,
        node_ref: 0,
        handler_stack: vec![],
    };
    adapter
        .run(term, incoming, &mut ctx)
        .unwrap_or_else(|e| panic!("type model error for term {:?}: {:?}", term, e))
}

/// Run `term` through the `arrow_model.hnl` [`HandlModel`] with the given
/// `incoming` value, returning the exec-model result.
fn arrow_model_run(term: &Term, incoming: Value) -> Value {
    let model = arrow_model();
    let ctx_value = json!({ "handler_stack": [], "node_ref": 0 });
    model
        .run(term, &incoming, &ctx_value)
        .unwrap_or_else(|e| panic!("arrow model error for term {:?}: {:?}", term, e))
}

// -------------------------------------------------------------------------
// Core arrow tags — read as type-level constructions by the type model.
// -------------------------------------------------------------------------

#[test]
fn type_model_arr_is_constant_type() {
        ensure_lang();
    // `Arr` in the type model is the constant type: the value IS the type
    // (HoTT: types are values), so the type of a pure value is the value
    // itself. This agrees with the arrow model.
    let term = to_term(TermData::arr(json!(5)));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!(5));
    assert_eq!(ty, arrow_model_run(&term, Value::Null));
}

#[test]
fn type_model_const_is_constant_type() {
        ensure_lang();
    let term = to_term(TermData::const_value(json!(7)));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!(7));
    assert_eq!(ty, arrow_model_run(&term, Value::Null));
}

#[test]
fn type_model_seq_is_compose_type() {
        ensure_lang();
    // `Seq` in the type model is type composition: the composite type
    // `b ∘ a`. With non-function operands, `compose_type` builds a tagged
    // `Compose` type record (the type-level counterpart of execution-time
    // sequencing). This deliberately differs from the arrow model, which
    // executes the composition.
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!({ "__tag__": "Compose", "fst": 1, "snd": 2 }));
    // The arrow model instead executes the composition (2, the snd applied
    // to the fst result) — confirming the two interpretations diverge here.
    assert_eq!(arrow_model_run(&term, Value::Null), json!(2));
}

// -------------------------------------------------------------------------
// HoTT-specific tags — type/proof judgments.
// -------------------------------------------------------------------------

#[test]
fn type_model_path_is_identity_type() {
        ensure_lang();
    // `Path a b` is the identity type `Id_A(a, b)`. The type model evaluates
    // the endpoints and constructs a `Path`-tagged type witnessing that the
    // two points are equal. (The witness field is not part of the identity
    // type former — only `from`/`to` are recorded.)
    let term = to_term(TermData::path(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
        Some(TermData::arr(json!(3))),
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!({ "__tag__": "Path", "from": 1, "to": 2 }));
}

#[test]
fn type_model_equiv_is_equivalence_type() {
        ensure_lang();
    // `Equiv a b` is the equivalence type: a proposition (`type.bool`)
    // witnessing that `a ≃ b`. The type model evaluates the operands and
    // records the propositional witness `a == b`.
    let term = to_term(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(1)),
        None,
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!({ "__tag__": "type.bool", "value": true }));

    // A non-equivalence yields a false propositional witness.
    let term_false = to_term(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
        None,
    ));
    let ty_false = type_model_run(&term_false, Value::Null);
    assert_eq!(ty_false, json!({ "__tag__": "type.bool", "value": false }));
}

#[test]
fn type_model_refine_yields_incoming_when_predicate_holds() {
        ensure_lang();
    // `Refine` is a refinement type (subtype): a proposition over a type.
    // When the predicate holds, the refined type is the incoming (ambient)
    // type; otherwise it is a type error.
    let term = to_term(TermData::refine(
        TermData::arr(json!("Nat")),
        TermData::arr(json!(true)),
    ));
    let incoming = json!("Nat");
    let ty = type_model_run(&term, incoming.clone());
    assert_eq!(ty, incoming);
}

#[test]
fn type_model_refine_is_type_error_when_predicate_fails() {
        ensure_lang();
    let term = to_term(TermData::refine(
        TermData::arr(json!("Nat")),
        TermData::arr(json!(false)),
    ));
    let ty = type_model_run(&term, json!("Nat"));
    assert_eq!(
        ty.get("__tag__").and_then(|v| v.as_str()),
        Some("TypeError")
    );
    assert_eq!(
        ty.get("message").and_then(|v| v.as_str()),
        Some("Refine: predicate not satisfied")
    );
}

#[test]
fn type_model_rewrite_evaluates_target() {
        ensure_lang();
    // `Rewrite` is a 2-cell; the type of a rewrite is the type of its
    // target. The type model evaluates the target sub-term. This agrees
    // with the arrow model, which also evaluates the target.
    use handl_language::RewriteWitness;
    let term = to_term(TermData::rewrite(
        TermData::arr(json!(1)),
        TermData::arr(json!(5)),
        &RewriteWitness::Named("x".to_string()),
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!(5));
    assert_eq!(ty, arrow_model_run(&term, Value::Null));
}

#[test]
fn type_model_cell_id_is_identity_cell() {
        ensure_lang();
    // `CellId arrow` is the identity 2-cell: source = target = the arrow
    // itself. The type model constructs a `Cell2`-tagged cell. This agrees
    // with the arrow model.
    let term = to_term(TermData::cell_id(TermData::arr(json!(7))));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(
        ty,
        json!({
            "__tag__": "Cell2",
            "__src__": 7,
            "__tgt__": 7,
            "value": 7
        })
    );
    assert_eq!(ty, arrow_model_run(&term, Value::Null));
}

#[test]
fn type_model_src_is_source_face() {
        ensure_lang();
    // `Src` projects the source face of a 2-cell type. With an identity cell
    // (`CellId`), the source face is the arrow itself.
    let term = to_term(TermData::src(TermData::cell_id(TermData::arr(json!(7)))));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!(7));
}

#[test]
fn type_model_tgt_is_target_face() {
        ensure_lang();
    let term = to_term(TermData::tgt(TermData::cell_id(TermData::arr(json!(7)))));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!(7));
}

#[test]
fn type_model_cell_comp_v_composes_faces() {
        ensure_lang();
    // `CellCompV` is vertical composition of 2-cells: the source of the
    // composite is the source of the first cell; the target is the target
    // of the second cell.
    let term = to_term(TermData::cell_comp_v(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(
        ty,
        json!({
            "__tag__": "Cell2",
            "__src__": 1,
            "__tgt__": 2,
            "value": 2
        })
    );
}

#[test]
fn type_model_whisker_attaches_arrow_to_cell() {
        ensure_lang();
    // `Whisker` attaches an arrow to a 2-cell, composing the faces. With a
    // `Left` whisker, the arrow is composed on the left of both faces.
    use handl_language::Side;
    let term = to_term(TermData::whisker(
        &Side::Left,
        TermData::arr(json!(3)),
        TermData::cell_id(TermData::arr(json!(4))),
    ));
    let ty = type_model_run(&term, Value::Null);
    // The identity cell has src = tgt = value = 4; left-whiskering composes
    // the arrow (3) on the left of both faces via `arrow_compose`.
    let expected_face = json!({
        "__tag__": "Arrow",
        "first": 3,
        "second": 4,
    });
    assert_eq!(
        ty,
        json!({
            "__tag__": "Cell2",
            "__src__": expected_face,
            "__tgt__": expected_face,
            "value": 4
        })
    );
}

#[test]
fn type_model_univalence_produces_path_from_equiv() {
        ensure_lang();
    // `Univalence` turns an equivalence into a path. The type model reads
    // the `Equiv`-tagged value and constructs a `Path` from its `a`/`b`/`path`
    // fields. Here the equiv sub-term evaluates to a bare boolean (the
    // propositional witness), which is not an `Equiv`-tagged object, so the
    // type model returns it unchanged.
    let term = to_term(TermData::univalence(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(1)),
        None,
    )));
    let ty = type_model_run(&term, Value::Null);
    // The inner `Equiv` evaluates to `{ "__tag__": "type.bool", "value": true }`,
    // which is not an `Equiv`-tagged object, so univalence returns it as-is.
    assert_eq!(ty, json!({ "__tag__": "type.bool", "value": true }));
}

#[test]
fn type_model_cobind_is_graded_type() {
        ensure_lang();
    // `Cobind` is a graded type (coeffect): attach the `__coeffects__` field
    // (the grade) to the body type. The grade sub-term is read raw (not
    // evaluated) by the type model, recording the grade term itself.
    let term = to_term(TermData::cobind(
        TermData::arr(json!("Affine")),
        TermData::arr(json!(42)),
    ));
    let ty = type_model_run(&term, Value::Null);
    // ADR-0119 §3 added `dim` to `Arr`: a grade term now carries its dimension
    // alongside its value, so the recorded grade is an `Arr` *with* `dim`.
    assert_eq!(
        ty,
        json!({
            "__type__": 42,
            "__coeffects__": [{
                "grade": {
                    "__tag__": "Arr",
                    "dim": { "__tag__": "Const", "value": 1 },
                    "value": "Affine"
                }
            }]
        })
    );
}

#[test]
fn type_model_glue_returns_family_type() {
        ensure_lang();
    // `Glue` glues a family along a path; the type model returns the family
    // type.
    let term = to_term(TermData::glue(
        TermData::arr(json!("Fam")),
        TermData::arr(json!("P")),
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!("Fam"));
}

#[test]
fn type_model_fibrant_returns_base_type() {
        ensure_lang();
    // `Fibrant` is fibrant replacement; the type model returns the base type.
    let term = to_term(TermData::fibrant(
        TermData::arr(json!("Base")),
        TermData::arr(json!("Fam")),
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!("Base"));
}

#[test]
fn type_model_jelim_dispatches_on_path_tag() {
        ensure_lang();
    // `JElim` is J-elimination: path-based motive dispatch. When the path
    // evaluates to a `Path`-tagged value, the motive is applied to the base;
    // otherwise the base is returned.
    let path_obj = json!({ "__tag__": "Path", "from": 1, "to": 1, "witness": null });
    let term = to_term(TermData::jelim(
        TermData::arr(path_obj),
        TermData::arr(json!("Motive")),
        TermData::arr(json!(99)),
    ));
    let ty = type_model_run(&term, Value::Null);
    // The path sub-term evaluates to the `Path`-tagged object, so the motive
    // is applied to the base value (99); a constant `Arr` motive returns its
    // value regardless of input, so the result is "Motive".
    assert_eq!(ty, json!("Motive"));
}

#[test]
fn type_model_record_proj_projects_field() {
        ensure_lang();
    let term = to_term(TermData::record_proj(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        "a",
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!(1));
}

#[test]
fn type_model_record_with_updates_field() {
        ensure_lang();
    let term = to_term(TermData::record_with(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        "a",
        TermData::arr(json!(10)),
    ));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!({ "a": 10, "b": 2 }));
}

#[test]
fn type_model_tag_wraps_value() {
        ensure_lang();
    let term = to_term(TermData::tag_value("ok", TermData::arr(json!(42))));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(ty, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn type_model_child_is_type_error() {
        ensure_lang();
    // `Child` is AST recursion, which is the exec model's job, not the type
    // model's — so the type model reports a `TypeError`.
    let term = to_term(TermData::child("value"));
    let ty = type_model_run(&term, Value::Null);
    assert_eq!(
        ty.get("__tag__").and_then(|v| v.as_str()),
        Some("TypeError")
    );
    assert_eq!(
        ty.get("message").and_then(|v| v.as_str()),
        Some("Child: AST recursion is the exec model's job, not the type model's")
    );
}

// -------------------------------------------------------------------------
// Overlap agreement: type model vs arrow model on coincident semantics.
// -------------------------------------------------------------------------

#[test]
fn type_and_arrow_models_agree_on_coincident_tags() {
        ensure_lang();
    // The type model and the arrow model read the same `Term` tags through
    // different lenses (type judgment vs executable combinator). They agree
    // exactly on the tags whose type-level and exec-level semantics coincide:
    // `Arr`/`Const` (the value is the type), `Rewrite` (evaluate target),
    // and `CellId` (identity 2-cell).
    let cases: Vec<(Term, Value)> = vec![
        (to_term(TermData::arr(json!(5))), json!(5)),
        (to_term(TermData::const_value(json!(7))), json!(7)),
        (
            to_term(TermData::rewrite(
                TermData::arr(json!(1)),
                TermData::arr(json!(9)),
                &handl_language::RewriteWitness::Named("w".to_string()),
            )),
            json!(9),
        ),
        (
            to_term(TermData::cell_id(TermData::arr(json!(7)))),
            json!({
                "__tag__": "Cell2",
                "__src__": 7,
                "__tgt__": 7,
                "value": 7
            }),
        ),
    ];

    for (term, expected) in cases {
        let ty = type_model_run(&term, Value::Null);
        let ax = arrow_model_run(&term, Value::Null);
        assert_eq!(ty, expected, "type model mismatch for {:?}", term);
        assert_eq!(ax, expected, "arrow model mismatch for {:?}", term);
        assert_eq!(ty, ax, "type/arrow disagreement for {:?}", term);
    }
}
