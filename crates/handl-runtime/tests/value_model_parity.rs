use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use hnl_engine_exec_tree_walk::ValueModel;
use handl_plugin_lib_handl::{HandlModel, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use hnl_engine::{ArrSource, AstNode, Model, ModelCtx, TermData};
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

const VALUE_MODEL_HNL: &str = include_str!("fixtures/value_model.hnl");

/// Convert a `TermData` back to `Term` for passing to `Model::run` (which
/// takes `&Term`). The conversion is lossless for all well-formed `TermData`.
fn to_term(td: TermData) -> Term {
    td.to_term().expect("TermData -> Term conversion failed")
}

fn handl_model() -> HandlModel {
    let module = HandlModule::new("value_model", VALUE_MODEL_HNL, &["run"])
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

fn rust_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(hnl_engine_exec_tree_walk::TreeWalkEngine::new().with_modules(modules))
}

fn eval(term: &Term, incoming: Value) -> (Value, Value) {
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
        rust.run(term, incoming.clone(), &mut ctx)
            .unwrap_or(Value::Null)
    };

    let handl = handl_model();
    let ctx_value = json!({ "handler_stack": [], "node_ref": 0 });
    let handl_value = handl
        .run(term, &incoming, &ctx_value)
        .unwrap_or_else(|e| panic!("handl model error for term {:?}: {:?}", term, e));

    (rust_value, handl_value)
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

#[test]
fn parity_arr_const() {
        ensure_lang();
    let term = to_term(TermData::arr(json!(5)));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, json!(5));
    assert_eq!(handl, json!(5));
    assert_eq!(rust, handl);
}

#[test]
fn parity_seq() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(2));
}

#[test]
fn parity_first() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(pair(json!(1), json!(2))),
        TermData::first(TermData::op("inc", vec![], &ArrSource::Native, None)),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, pair(json!(2), json!(2)));
}

#[test]
fn parity_choice_left() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(either_left(json!(5))),
        TermData::choice(
            TermData::op("inc", vec![], &ArrSource::Native, None),
            TermData::op("dec", vec![], &ArrSource::Native, None),
        ),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(6));
}

#[test]
fn parity_choice_right() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(either_right(json!(5))),
        TermData::choice(
            TermData::op("inc", vec![], &ArrSource::Native, None),
            TermData::op("dec", vec![], &ArrSource::Native, None),
        ),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(4));
}

#[test]
fn parity_loop() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(pair(json!(1), json!(2))),
        TermData::loop_feedback(TermData::arr(pair(json!(42), json!(0)))),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(42));
}

#[test]
fn parity_iter() {
        ensure_lang();
    let term = to_term(TermData::iter(
        TermData::arr(json!(10)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
        TermData::op("dec", vec![], &ArrSource::Native, None),
        TermData::arr(json!(3)),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(0));
}

#[test]
fn parity_app_tag_inspection() {
        ensure_lang();
    // `app` to a `tag.*` op through the module registry — post-deletion
    // the surviving op is `tag.untag` (the `tag.tag` producer is gone,
    // ADR-0152 §Migration): applied to a host-internal tagged record it
    // returns the payload.
    let term = to_term(TermData::app(
        TermData::arr(json!("tag.untag")),
        vec![TermData::arr(json!({ "__tag__": "ok", "value": 42 }))],
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!({ "value": 42 }));
}

#[test]
fn parity_record_proj() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        TermData::record_proj(TermData::op("id", vec![], &ArrSource::Native, None), "a"),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(1));
}

#[test]
fn parity_record_with() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        TermData::record_with(
            TermData::op("id", vec![], &ArrSource::Native, None),
            "a",
            TermData::arr(json!(10)),
        ),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!({ "a": 10, "b": 2 }));
}

#[test]
fn parity_tag() {
        ensure_lang();
    let term = to_term(TermData::tag_value("ok", TermData::arr(json!(42))));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn parity_refine() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(5)),
        TermData::refine(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("is_pos", vec![], &ArrSource::Native, None),
        ),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(5));
}

#[test]
fn parity_cobind() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(99)),
        TermData::cobind(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("id", vec![], &ArrSource::Native, None),
        ),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(99));
}

#[test]
fn parity_path() {
        ensure_lang();
    let term = to_term(TermData::path(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
        Some(TermData::arr(json!(3))),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
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
        ensure_lang();
    let term = to_term(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(1)),
        None,
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_equiv_path() {
        ensure_lang();
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
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_cell_id() {
        ensure_lang();
    let term = to_term(TermData::cell_id(TermData::arr(json!(7))));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
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
#[ignore = "model-parity gap: the Rust default model returns Null for derived (opaque) records that the Handl model evaluates. See project/TODO.md"]
fn parity_cell_comp_v() {
        ensure_lang();
    let term = to_term(TermData::cell_comp_v(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
}

#[test]
fn parity_whisker_left() {
        ensure_lang();
    use handl_language::Side;
    let term = to_term(TermData::whisker(
        &Side::Left,
        TermData::arr(json!(3)),
        TermData::cell_id(TermData::arr(json!(4))),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
}

#[test]
fn parity_univalence() {
        ensure_lang();
    let term = to_term(TermData::univalence(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(1)),
        None,
    )));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
}

#[test]
fn parity_glue_fibrant_jelim() {
        ensure_lang();
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
        let (rust, handl) = eval(&term, Value::Null);
        assert_eq!(rust, handl);
        assert_eq!(rust, json!(99));
    }
}

#[test]
fn parity_rewrite() {
        ensure_lang();
    use handl_language::RewriteWitness;
    let term = to_term(TermData::seq(
        TermData::arr(json!(5)),
        TermData::rewrite(
            TermData::arr(json!(1)),
            TermData::op("id", vec![], &ArrSource::Native, None),
            &RewriteWitness::Named("x".to_string()),
        ),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(5));
}

#[test]
fn parity_op_arithmetic() {
        ensure_lang();
    let term = to_term(TermData::op(
        "add",
        vec![TermData::arr(json!(1)), TermData::arr(json!(2))],
        &ArrSource::Native,
        None,
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(3));
}

#[test]
fn parity_op_is_pos() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(3)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_handle_number_add() {
        ensure_lang();
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
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, number_value(3, &["Addable"]));
}
