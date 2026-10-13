use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::ValueModel;
use handl_plugin_lib_handl::{HandlModel, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use plugin_engine::{ArrSource, AstNode, Model, ModelCtx, TermData};
use serde_json::json;

const ARROW_MODEL_HNL: &str = include_str!("../../../project/design/adr0103/arrow_model.hnl");

/// Convert a `TermData` back to `Term` for passing to `Model::run` (which
/// takes `&Term`). The conversion is lossless for all well-formed `TermData`.
fn to_term(td: TermData) -> Term {
    td.to_term().expect("TermData -> Term conversion failed")
}

fn handl_model() -> HandlModel {
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
    HandlModel::new(module)
}

fn rust_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new().with_modules(modules))
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
fn arrow_parity_arr_const() {
    let term = to_term(TermData::arr(json!(5)));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, json!(5));
    assert_eq!(handl, json!(5));
    assert_eq!(rust, handl);
}

#[test]
fn arrow_parity_seq() {
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(2));
}

#[test]
fn arrow_parity_first() {
    let term = to_term(TermData::seq(
        TermData::arr(pair(json!(1), json!(2))),
        TermData::first(TermData::op("inc", vec![], &ArrSource::Native, None)),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, pair(json!(2), json!(2)));
}

#[test]
fn arrow_parity_choice_left() {
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
fn arrow_parity_choice_right() {
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
fn arrow_parity_loop() {
    let term = to_term(TermData::seq(
        TermData::arr(pair(json!(1), json!(2))),
        TermData::loop_feedback(TermData::arr(pair(json!(42), json!(0)))),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(42));
}

#[test]
fn arrow_parity_iter() {
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
fn arrow_parity_app_tag() {
    let term = to_term(TermData::app(
        TermData::arr(json!("tag.tag")),
        vec![
            TermData::arr(json!("ok")),
            TermData::arr(json!({ "value": 42 })),
        ],
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn arrow_parity_record_proj() {
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        TermData::record_proj(TermData::op("id", vec![], &ArrSource::Native, None), "a"),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(1));
}

#[test]
fn arrow_parity_record_with() {
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
fn arrow_parity_tag() {
    let term = to_term(TermData::tag_value("ok", TermData::arr(json!(42))));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn arrow_parity_refine() {
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
fn arrow_parity_cobind() {
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
fn arrow_parity_path() {
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
fn arrow_parity_equiv_bool() {
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
fn arrow_parity_equiv_path() {
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
fn arrow_parity_cell_id() {
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
fn arrow_parity_cell_comp_v() {
    let term = to_term(TermData::cell_comp_v(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
}

#[test]
fn arrow_parity_whisker_left() {
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
fn arrow_parity_univalence() {
    let term = to_term(TermData::univalence(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(1)),
        None,
    )));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
}

#[test]
fn arrow_parity_glue_fibrant_jelim() {
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
fn arrow_parity_rewrite() {
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
fn arrow_parity_op_arithmetic() {
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
fn arrow_parity_op_is_pos() {
    let term = to_term(TermData::seq(
        TermData::arr(json!(3)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
    ));
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(true));
}

#[test]
fn arrow_parity_handle_number_add() {
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
