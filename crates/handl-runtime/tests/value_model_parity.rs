use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::ValueModel;
use handl_plugin_lib_handl::{HandlModel, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use plugin_engine::{AstNode, Model, ModelCtx};
use serde_json::json;

const VALUE_MODEL_HNL: &str = include_str!("../../../project/design/adr0103/value_model.hnl");

fn handl_model() -> HandlModel {
    let module = HandlModule::new("value_model", VALUE_MODEL_HNL, &["run"])
        .with_dependency(stdlib::module("array").expect("array module"))
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
fn parity_arr_const() {
    let term = Term::Arr { value: json!(5) };
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, json!(5));
    assert_eq!(handl, json!(5));
    assert_eq!(rust, handl);
}

#[test]
fn parity_seq() {
    let term = Term::seq(
        Term::Arr { value: json!(1) },
        Term::op_native("inc", vec![]),
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(2));
}

#[test]
fn parity_first() {
    let term = Term::seq(
        Term::Arr {
            value: pair(json!(1), json!(2)),
        },
        Term::first(Term::op_native("inc", vec![])),
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, pair(json!(2), json!(2)));
}

#[test]
fn parity_choice_left() {
    let term = Term::seq(
        Term::Arr {
            value: either_left(json!(5)),
        },
        Term::choice(
            Term::op_native("inc", vec![]),
            Term::op_native("dec", vec![]),
        ),
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(6));
}

#[test]
fn parity_choice_right() {
    let term = Term::seq(
        Term::Arr {
            value: either_right(json!(5)),
        },
        Term::choice(
            Term::op_native("inc", vec![]),
            Term::op_native("dec", vec![]),
        ),
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(4));
}

#[test]
fn parity_loop() {
    let term = Term::seq(
        Term::Arr {
            value: pair(json!(1), json!(2)),
        },
        Term::loop_feedback(Term::Arr {
            value: pair(json!(42), json!(0)),
        }),
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(42));
}

#[test]
fn parity_iter() {
    let term = Term::Iter {
        fuel: Box::new(Term::Arr { value: json!(10) }),
        predicate: Box::new(Term::op_native("is_pos", vec![])),
        step: Box::new(Term::op_native("dec", vec![])),
        initial: Box::new(Term::Arr { value: json!(3) }),
    };
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(0));
}

#[test]
fn parity_app_tag() {
    let term = Term::app(
        Term::Arr {
            value: json!("tag.tag"),
        },
        vec![
            Term::Arr { value: json!("ok") },
            Term::Arr {
                value: json!({ "value": 42 }),
            },
        ],
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn parity_record_proj() {
    let term = Term::seq(
        Term::Arr {
            value: json!({ "a": 1, "b": 2 }),
        },
        Term::RecordProj {
            record: Box::new(Term::op_native("id", vec![])),
            field: "a".to_string(),
        },
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(1));
}

#[test]
fn parity_record_with() {
    let term = Term::seq(
        Term::Arr {
            value: json!({ "a": 1, "b": 2 }),
        },
        Term::RecordWith {
            record: Box::new(Term::op_native("id", vec![])),
            field: "a".to_string(),
            value: Box::new(Term::Arr { value: json!(10) }),
        },
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!({ "a": 10, "b": 2 }));
}

#[test]
fn parity_tag() {
    let term = Term::Tag {
        tag: "ok".to_string(),
        value: Box::new(Term::Arr { value: json!(42) }),
    };
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn parity_refine() {
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::Refine {
            sort: Box::new(Term::op_native("id", vec![])),
            predicate: Box::new(Term::op_native("is_pos", vec![])),
        },
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(5));
}

#[test]
fn parity_cobind() {
    let term = Term::seq(
        Term::Arr { value: json!(99) },
        Term::Cobind {
            grade: Box::new(Term::op_native("id", vec![])),
            body: Box::new(Term::op_native("id", vec![])),
        },
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(99));
}

#[test]
fn parity_path() {
    let term = Term::Path {
        endpoints: (
            Box::new(Term::Arr { value: json!(1) }),
            Box::new(Term::Arr { value: json!(2) }),
        ),
        witness: Some(Box::new(Term::Arr { value: json!(3) })),
    };
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
    let term = Term::Equiv {
        a: Box::new(Term::Arr { value: json!(1) }),
        b: Box::new(Term::Arr { value: json!(1) }),
        path: None,
    };
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_equiv_path() {
    let term = Term::seq(
        Term::Arr { value: json!(1) },
        Term::Equiv {
            a: Box::new(Term::Arr { value: json!(1) }),
            b: Box::new(Term::op_native("id", vec![])),
            path: Some(Box::new(Term::Path {
                endpoints: (
                    Box::new(Term::Arr { value: json!(1) }),
                    Box::new(Term::Arr { value: json!(1) }),
                ),
                witness: None,
            })),
        },
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_cell_id() {
    let term = Term::CellId {
        arrow: Box::new(Term::Arr { value: json!(7) }),
    };
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
fn parity_cell_comp_v() {
    let term = Term::CellCompV {
        first: Box::new(Term::Arr { value: json!(1) }),
        second: Box::new(Term::Arr { value: json!(2) }),
    };
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
}

#[test]
fn parity_whisker_left() {
    use handl_language::Side;
    let term = Term::Whisker {
        side: Side::Left,
        arrow: Box::new(Term::Arr { value: json!(3) }),
        cell: Box::new(Term::CellId {
            arrow: Box::new(Term::Arr { value: json!(4) }),
        }),
    };
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
}

#[test]
fn parity_univalence() {
    let term = Term::Univalence {
        equiv: Box::new(Term::Equiv {
            a: Box::new(Term::Arr { value: json!(1) }),
            b: Box::new(Term::Arr { value: json!(1) }),
            path: None,
        }),
    };
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
}

#[test]
fn parity_glue_fibrant_jelim() {
    // `Glue`, `Fibrant`, and `JElim` all return the incoming/base value when
    // their path/family arguments do not provide a genuine path/cell.
    let glue = Term::seq(
        Term::Arr { value: json!(99) },
        Term::Glue {
            path: Box::new(Term::op_native("id", vec![])),
            family: Box::new(Term::op_native("id", vec![])),
        },
    );
    let fibrant = Term::seq(
        Term::Arr { value: json!(99) },
        Term::Fibrant {
            base: Box::new(Term::op_native("id", vec![])),
            family: Box::new(Term::op_native("id", vec![])),
        },
    );
    let path_obj = json!({ "__tag__": "Path", "from": 1, "to": 1, "witness": null });
    let jelim = Term::seq(
        Term::Arr { value: path_obj },
        Term::JElim {
            path: Box::new(Term::op_native("id", vec![])),
            motive: Box::new(Term::op_native("id", vec![])),
            base: Box::new(Term::Arr { value: json!(99) }),
        },
    );
    for term in [glue, fibrant, jelim] {
        let (rust, handl) = eval(&term, Value::Null);
        assert_eq!(rust, handl);
        assert_eq!(rust, json!(99));
    }
}

#[test]
fn parity_rewrite() {
    use handl_language::RewriteWitness;
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::Rewrite {
            source: Box::new(Term::Arr { value: json!(1) }),
            target: Box::new(Term::op_native("id", vec![])),
            witness: RewriteWitness::Named("x".to_string()),
        },
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(5));
}

#[test]
fn parity_op_arithmetic() {
    let term = Term::op_native(
        "add",
        vec![Term::Arr { value: json!(1) }, Term::Arr { value: json!(2) }],
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(3));
}

#[test]
fn parity_op_is_pos() {
    let term = Term::seq(
        Term::Arr { value: json!(3) },
        Term::op_native("is_pos", vec![]),
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, json!(true));
}

#[test]
fn parity_handle_number_add() {
    use handl_language::ArrSource;
    let handler_impl = serde_json::to_value(&Term::Op {
        name: "number_add".to_string(),
        args: vec![],
        source: ArrSource::Native,
        lit: None,
    })
    .unwrap();
    let handler = json!({
        "__tag__": "Handler",
        "grades": ["Addable"],
        "impls": { "add": handler_impl }
    });
    let term = Term::seq(
        Term::Arr {
            value: number_value(0, &["Addable"]),
        },
        Term::handle(
            Term::op_native(
                "add",
                vec![
                    Term::Arr {
                        value: number_value(1, &["Addable"]),
                    },
                    Term::Arr {
                        value: number_value(2, &["Addable"]),
                    },
                ],
            ),
            "Number",
            Term::Arr { value: handler },
        ),
    );
    let (rust, handl) = eval(&term, Value::Null);
    assert_eq!(rust, handl);
    assert_eq!(rust, number_value(3, &["Addable"]));
}
