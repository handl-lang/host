//! ADR-0103 Phase 4: composite model parity tests.
//!
//! These tests verify that a [`CompositeModel`] — delegating HoTT-specific
//! tags to a Handl-authored model (data) via [`HandlModelAdapter`] while
//! keeping core arrow-calculus tags in the Rust [`ValueModel`] (bootstrap) —
//! produces the same results as the pure Rust [`ValueModel`] for all `Term`
//! variants.
//!
//! This is the "models as data" architecture: the composite model is
//! interchangeable with the pure Rust model, demonstrating that HoTT
//! semantics can live in `.hnl` data without changing the kernel.

use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::ValueModel;
use handl_plugin_lib_handl::{HandlModelAdapter, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use plugin_engine::{ArrSource, AstNode, CompositeModel, Model, ModelCtx, TermData, is_hott_tag};
use serde_json::json;

const ARROW_MODEL_HNL: &str = include_str!("../../../project/design/adr0103/arrow_model.hnl");

/// Convert a `TermData` back to `Term` for passing to `Model::run` (which
/// takes `&Term`). The conversion is lossless for all well-formed `TermData`.
fn to_term(td: TermData) -> Term {
    td.to_term().expect("TermData -> Term conversion failed")
}

/// Build a `HandlModelAdapter` from the `arrow_model.hnl` module.
fn handl_adapter() -> HandlModelAdapter {
    let module = HandlModule::new("arrow_model", ARROW_MODEL_HNL, &["run"])
        .with_dependency_arc(
            stdlib::load_hnl_file(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../handl-packages/collections/array/index.hnl"
            ))
            .expect("array module"),
        )
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule);
    HandlModelAdapter::new(module)
}

/// Build the pure Rust `ValueModel` (the bootstrap engine).
fn rust_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new().with_modules(modules))
}

/// Build the composite model: `ValueModel` (primary) + `HandlModelAdapter`
/// (delegate), using `is_hott_tag` as the delegation predicate.
fn composite_model() -> CompositeModel<ValueModel, HandlModelAdapter> {
    CompositeModel::new(rust_model(), handl_adapter(), is_hott_tag)
}

/// Evaluate `term` with both the composite model and the pure Rust model,
/// returning `(composite_value, rust_value)`.
fn eval(term: &Term, incoming: Value) -> (Value, Value) {
    let composite = composite_model();
    let composite_value = {
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
        composite
            .run(term, incoming.clone(), &mut ctx)
            .unwrap_or(Value::Null)
    };

    let rust = rust_model();
    let rust_value = {
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
        rust.run(term, incoming, &mut ctx).unwrap_or(Value::Null)
    };

    (composite_value, rust_value)
}

fn number_value(value: i64, grades: &[&str]) -> Value {
    json!({
        "__tag__": "Number",
        "grades": grades,
        "value": value
    })
}

fn pair(a: Value, b: Value) -> Value {
    json!({ "__pair__": [a, b] })
}

fn either_left(v: Value) -> Value {
    json!({ "__either__": "left", "value": v })
}

fn either_right(v: Value) -> Value {
    json!({ "__either__": "right", "value": v })
}

// -------------------------------------------------------------------------
// Core arrow-calculus tags (handled by the primary Rust ValueModel)
// -------------------------------------------------------------------------

#[test]
fn parity_arr_const() {
    let term = to_term(TermData::arr(json!(5)));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(rust, json!(5));
    assert_eq!(composite, rust);
}

#[test]
fn parity_seq() {
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(2));
}

#[test]
fn parity_first() {
    let term = to_term(TermData::seq(
        TermData::arr(pair(json!(1), json!(2))),
        TermData::first(TermData::op("inc", vec![], &ArrSource::Native, None)),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, pair(json!(2), json!(2)));
}

#[test]
fn parity_choice_left() {
    let term = to_term(TermData::seq(
        TermData::arr(either_left(json!(5))),
        TermData::choice(
            TermData::op("inc", vec![], &ArrSource::Native, None),
            TermData::op("dec", vec![], &ArrSource::Native, None),
        ),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(6));
}

#[test]
fn parity_choice_right() {
    let term = to_term(TermData::seq(
        TermData::arr(either_right(json!(5))),
        TermData::choice(
            TermData::op("inc", vec![], &ArrSource::Native, None),
            TermData::op("dec", vec![], &ArrSource::Native, None),
        ),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(4));
}

#[test]
fn parity_loop() {
    let term = to_term(TermData::seq(
        TermData::arr(pair(json!(1), json!(2))),
        TermData::loop_feedback(TermData::arr(pair(json!(42), json!(0)))),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(42));
}

#[test]
fn parity_iter() {
    let term = to_term(TermData::iter(
        TermData::arr(json!(10)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
        TermData::op("dec", vec![], &ArrSource::Native, None),
        TermData::arr(json!(3)),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(0));
}

#[test]
fn parity_app_tag() {
    let term = to_term(TermData::app(
        TermData::arr(json!("tag.tag")),
        vec![
            TermData::arr(json!("ok")),
            TermData::arr(json!({ "value": 42 })),
        ],
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn parity_record_proj() {
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        TermData::record_proj(TermData::op("id", vec![], &ArrSource::Native, None), "a"),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(1));
}

#[test]
fn parity_record_with() {
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        TermData::record_with(
            TermData::op("id", vec![], &ArrSource::Native, None),
            "a",
            TermData::arr(json!(10)),
        ),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!({ "a": 10, "b": 2 }));
}

#[test]
fn parity_tag() {
    let term = to_term(TermData::tag_value("ok", TermData::arr(json!(42))));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!({ "__tag__": "ok", "value": 42 }));
}

// -------------------------------------------------------------------------
// HoTT-specific tags (delegated to the HandlModelAdapter)
// -------------------------------------------------------------------------

#[test]
fn parity_refine() {
    let term = to_term(TermData::seq(
        TermData::arr(json!(5)),
        TermData::refine(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("is_pos", vec![], &ArrSource::Native, None),
        ),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(5));
}

#[test]
fn parity_cobind() {
    let term = to_term(TermData::seq(
        TermData::arr(json!(99)),
        TermData::cobind(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("id", vec![], &ArrSource::Native, None),
        ),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(99));
}

#[test]
fn parity_path() {
    let term = to_term(TermData::path(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
        Some(TermData::arr(json!(3))),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(
        rust,
        json!({
            "__tag__": "Path",
            "from": 1,
            "to": 2,
            "witness": 3
        })
    );
}

#[test]
fn parity_equiv_bool() {
    let term = to_term(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(1)),
        None,
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_equiv_path() {
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::equiv(
            TermData::arr(json!(1)),
            TermData::op("id", vec![], &ArrSource::Native, None),
            Some(TermData::path(
                TermData::arr(json!(1)),
                TermData::arr(json!(1)),
                None,
            )),
        ),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_cell_id() {
    let term = to_term(TermData::cell_id(TermData::arr(json!(7))));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(
        rust,
        json!({
            "__tag__": "Cell2",
            "__src__": 7,
            "__tgt__": 7,
            "value": 7
        })
    );
}

#[test]
fn parity_cell_comp_v() {
    let term = to_term(TermData::cell_comp_v(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
}

#[test]
fn parity_whisker_left() {
    use handl_language::Side;
    let term = to_term(TermData::whisker(
        &Side::Left,
        TermData::arr(json!(3)),
        TermData::cell_id(TermData::arr(json!(4))),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
}

#[test]
fn parity_univalence() {
    let term = to_term(TermData::univalence(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(1)),
        None,
    )));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
}

#[test]
fn parity_glue_fibrant_jelim() {
    // `Glue`, `Fibrant`, and `JElim` all return the incoming/base value when
    // their path/family arguments do not provide a genuine path/cell.
    let glue = to_term(TermData::seq(
        TermData::arr(json!(99)),
        TermData::glue(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("id", vec![], &ArrSource::Native, None),
        ),
    ));
    let fibrant = to_term(TermData::seq(
        TermData::arr(json!(99)),
        TermData::fibrant(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("id", vec![], &ArrSource::Native, None),
        ),
    ));
    let path_obj = json!({ "__tag__": "Path", "from": 1, "to": 1, "witness": null });
    let jelim = to_term(TermData::seq(
        TermData::arr(path_obj),
        TermData::jelim(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::arr(json!(99)),
        ),
    ));
    for term in [glue, fibrant, jelim] {
        let (composite, rust) = eval(&term, Value::Null);
        assert_eq!(composite, rust);
        assert_eq!(rust, json!(99));
    }
}

#[test]
fn parity_rewrite() {
    use handl_language::RewriteWitness;
    let term = to_term(TermData::seq(
        TermData::arr(json!(5)),
        TermData::rewrite(
            TermData::arr(json!(1)),
            TermData::op("id", vec![], &ArrSource::Native, None),
            &RewriteWitness::Named("x".to_string()),
        ),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(5));
}

// -------------------------------------------------------------------------
// Op tags (handled by the primary Rust ValueModel)
// -------------------------------------------------------------------------

#[test]
fn parity_op_arithmetic() {
    let term = to_term(TermData::op(
        "add",
        vec![TermData::arr(json!(1)), TermData::arr(json!(2))],
        &ArrSource::Native,
        None,
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(3));
}

#[test]
fn parity_op_is_pos() {
    let term = to_term(TermData::seq(
        TermData::arr(json!(3)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_handle_number_add() {
    let handler_impl = serde_json::to_value(&to_term(TermData::op(
        "number_add",
        vec![],
        &ArrSource::Native,
        None,
    )))
    .unwrap();
    let handler = json!({
        "__tag__": "Handler",
        "grades": ["Addable"],
        "impls": { "add": handler_impl }
    });
    let term = to_term(TermData::seq(
        TermData::arr(number_value(0, &["Addable"])),
        TermData::handle(
            TermData::op(
                "add",
                vec![
                    TermData::arr(number_value(1, &["Addable"])),
                    TermData::arr(number_value(2, &["Addable"])),
                ],
                &ArrSource::Native,
                None,
            ),
            "Number",
            TermData::arr(handler),
        ),
    ));
    let (composite, rust) = eval(&term, Value::Null);
    assert_eq!(composite, rust);
    assert_eq!(rust, number_value(3, &["Addable"]));
}
