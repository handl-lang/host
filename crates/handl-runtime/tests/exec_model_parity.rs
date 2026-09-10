//! ADR-0103 Step 7: exec model parity tests.
//!
//! `exec_model.hnl` is the *exec model* — the Handl layer that defines how to
//! evaluate AST nodes via structural ops (`proj_child`, `eval_block`,
//! `extend_env`, `closure`, `pattern_match`, `apply`, `record`,
//! `field_access`, `tag_construct`, `number_add`, `eval`, …). It is the first
//! layer of Handl code on top of the VM: the structural ops that recurse
//! through the AST and the eval-scheme dispatch.
//!
//! These tests wrap the exec model module in a [`HandlModelAdapter`] (the
//! ADR-0103 "models as data" seam) and verify:
//!
//! 1. **Structural data operations** — `record`, `field_access`, `array`,
//!    `tag_construct`, `arrow_field`, `number_add` — called directly through
//!    the `Module`/`Function` trait. These are the operations that can be
//!    tested on the current tree-walk runtime because they only depend on
//!    stdlib modules (`hash_map`, `array`, `typeof`, `tag`).
//!
//! 2. **`run` through the `Model` trait** — the exec model's `run(term,
//!    incoming, ctx)` arrow dispatches on `__tag__` using the structural ops
//!    where available. Tags that overlap with `value_model.hnl` (the
//!    tree-walk engine) are compared for parity.
//!
//! ## Skipped operations (VM primitives not yet available)
//!
//! The following structural ops require genuine VM instruction-set primitives
//! (`push_scope`, `pop_scope`, `bind`, `lookup`, `call`, `ffi`,
//! `make_closure`, `pattern_match`, `lookup_eval_scheme`) that are not yet
//! implemented on the tree-walk runtime. They are defined in
//! `exec_model.hnl` (for documentation and future compilation) but are not
//! tested here:
//!
//! - `proj_child` / `proj_child_or` / `proj_rest` — require `eval` /
//!   `lookup_eval_scheme`.
//! - `eval_block` / `eval_rec` — require `push_scope` / `pop_scope` / `eval`.
//! - `proj_env` — requires `lookup`.
//! - `extend_env` — requires `push_scope` / `bind`.
//! - `closure` — requires `make_closure`.
//! - `pattern_match` — requires the VM `pattern_match` primitive.
//! - `push_handler` / `pop_handler` — require `ctx` binding.
//! - `apply` — requires `call`.
//! - `call_op` — requires `ctx` / `ffi` / `apply`.
//! - `import_module` / `export_binding` — require `ffi` / `bind` / `ctx`.
//! - `eval` — requires `lookup_eval_scheme`.

use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use handl_platform::{EvalContext, Module};
use handl_plugin_engine_exec_tree_walk::ValueModel;
use handl_plugin_lib_handl::{HandlModel, HandlModelAdapter, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use plugin_engine::{AstNode, Model, ModelCtx};
use serde_json::json;

const EXEC_MODEL_HNL: &str = include_str!("../../../project/design/adr0103/exec_model.hnl");
const VALUE_MODEL_HNL: &str = include_str!("../../../project/design/adr0103/value_model.hnl");

/// All function names exported by the exec model module (structural ops +
/// `run`).
const EXEC_MODEL_FUNCS: &[&str] = &[
    "proj_child",
    "proj_child_or",
    "proj_rest",
    "eval_block",
    "eval_rec",
    "proj_env",
    "extend_env",
    "closure",
    "pattern_match",
    "push_handler",
    "pop_handler",
    "apply",
    "record",
    "field_access",
    "array",
    "tag_construct",
    "arrow_field",
    "call_op",
    "import_module",
    "export_binding",
    "number_add",
    "eval",
    "run",
];

/// Build a `HandlModule` for the exec model with the stdlib dependency chain.
fn exec_model_module() -> HandlModule {
    HandlModule::new("exec_model", EXEC_MODEL_HNL, EXEC_MODEL_FUNCS)
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule)
}

/// Build a `HandlModelAdapter` wrapping the exec model module.
fn exec_model_adapter() -> HandlModelAdapter {
    HandlModelAdapter::new(exec_model_module())
}

/// Build a `HandlModel` backed by `value_model.hnl` (the tree-walk Handl
/// reference), used for parity comparison on overlapping tags.
fn value_model() -> HandlModel {
    let module = HandlModule::new("value_model", VALUE_MODEL_HNL, &["run"])
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule);
    HandlModel::new(module)
}

/// Build the pure Rust tree-walk `ValueModel` (the exec engine reference).
fn rust_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new().with_modules(modules))
}

/// Call a structural op by name on the exec model module, returning the
/// result.
fn call_op(name: &str, args: &[Value]) -> Value {
    let module = exec_model_module();
    let func = module
        .function(name)
        .unwrap_or_else(|| panic!("exec model has no '{}' function", name));
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
        .unwrap_or_else(|e| panic!("exec model '{}' error: {:?}", name, e))
}

/// Run `term` through the exec model [`HandlModelAdapter`] (via the `Model`
/// trait) with the given `incoming` value.
fn exec_model_run(term: &Term, incoming: Value) -> Value {
    let adapter = exec_model_adapter();
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
        .unwrap_or_else(|e| panic!("exec model run error for term {:?}: {:?}", term, e))
}

/// Run `term` through the Rust tree-walk `ValueModel` with the given
/// `incoming` value.
fn rust_run(term: &Term, incoming: Value) -> Value {
    let rust = rust_model();
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
    rust.run(term, incoming, &mut ctx)
        .unwrap_or(Value::Null)
}

/// Run `term` through the `value_model.hnl` [`HandlModel`] with the given
/// `incoming` value.
fn value_model_run(term: &Term, incoming: Value) -> Value {
    let model = value_model();
    let ctx_value = json!({ "handler_stack": [], "node_ref": 0 });
    model
        .run(term, &incoming, &ctx_value)
        .unwrap_or_else(|e| panic!("value model error for term {:?}: {:?}", term, e))
}

/// A tagged Number value (matching the `value_model.hnl` prototype shape).
fn number_value(value: i64, grades: &[&str]) -> Value {
    json!({
        "__tag__": "Number",
        "grades": grades,
        "value": value
    })
}

// ===========================================================================
// Part 1: Structural data operations (called directly via Module/Function)
// ===========================================================================

#[test]
fn exec_model_number_add_array_of_two() {
    // number_add on an array of two tagged Numbers: untag, add, re-tag.
    let args = vec![json!([
        number_value(1, &["Addable"]),
        number_value(2, &["Addable"])
    ])];
    let result = call_op("number_add", &args);
    assert_eq!(result, number_value(3, &["Addable"]));
}

#[test]
fn exec_model_number_add_pair() {
    // number_add on a serialized pair: the second component is the operands
    // array.
    let args = vec![json!({
        "__pair__": [json!(0), [number_value(10, &["Addable"]), number_value(5, &["Addable"])]]
    })];
    let result = call_op("number_add", &args);
    assert_eq!(result, number_value(15, &["Addable"]));
}

#[test]
fn exec_model_number_add_matches_value_model() {
    // The exec model's number_add should produce the same result as the
    // value_model's number_add (both use the tag.untag / tag.tag pattern).
    let operands = json!([number_value(7, &["Addable"]), number_value(8, &["Addable"])]);
    let exec_result = call_op("number_add", &[operands.clone()]);

    // Run number_add through the value model by constructing an Op term.
    let term = Term::op_native(
        "number_add",
        vec![
            Term::Arr {
                value: number_value(7, &["Addable"]),
            },
            Term::Arr {
                value: number_value(8, &["Addable"]),
            },
        ],
    );
    let vm_result = value_model_run(&term, Value::Null);

    assert_eq!(exec_result, number_value(15, &["Addable"]));
    assert_eq!(vm_result, number_value(15, &["Addable"]));
    assert_eq!(exec_result, vm_result);
}

#[test]
fn exec_model_record_construction() {
    // record: construct a record from field/value pairs.
    let args = vec![json!([
        ["a", 1],
        ["b", 2],
        ["c", 3]
    ])];
    let result = call_op("record", &args);
    assert_eq!(result, json!({ "a": 1, "b": 2, "c": 3 }));
}

#[test]
fn exec_model_record_empty() {
    let result = call_op("record", &[json!([])]);
    assert_eq!(result, json!({}));
}

#[test]
fn exec_model_field_access_record() {
    // field_access on a record (object): field lookup.
    let args = vec![json!({ "a": 1, "b": 2 }), json!("a")];
    let result = call_op("field_access", &args);
    assert_eq!(result, json!(1));
}

#[test]
fn exec_model_field_access_record_missing() {
    // field_access on a record with an absent field: returns null.
    let args = vec![json!({ "a": 1 }), json!("z")];
    let result = call_op("field_access", &args);
    assert_eq!(result, Value::Null);
}

#[test]
fn exec_model_field_access_array() {
    // field_access on an array: index lookup.
    let args = vec![json!([10, 20, 30]), json!(1)];
    let result = call_op("field_access", &args);
    assert_eq!(result, json!(20));
}

#[test]
fn exec_model_field_access_non_record_non_array() {
    // field_access on a non-record/non-array value: returns null.
    let args = vec![json!(42), json!("a")];
    let result = call_op("field_access", &args);
    assert_eq!(result, Value::Null);
}

#[test]
fn exec_model_array_identity() {
    // array: identity — returns the elements as-is.
    let args = vec![json!([1, 2, 3])];
    let result = call_op("array", &args);
    assert_eq!(result, json!([1, 2, 3]));
}

#[test]
fn exec_model_tag_construct() {
    // tag_construct: merge __tag__ with the payload fields (via tag.tag).
    let args = vec![json!("ok"), json!({ "value": 42 })];
    let result = call_op("tag_construct", &args);
    assert_eq!(result, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn exec_model_tag_construct_with_extra_fields() {
    // tag_construct merges all payload fields, not just "value".
    let args = vec![
        json!("Number"),
        json!({ "value": 5, "grades": ["Addable"] }),
    ];
    let result = call_op("tag_construct", &args);
    assert_eq!(
        result,
        json!({ "__tag__": "Number", "value": 5, "grades": ["Addable"] })
    );
}

#[test]
fn exec_model_arrow_field() {
    // arrow_field: project a named field from an arrow cell record.
    let arrow = json!({ "__tag__": "Arrow", "first": 1, "second": 2 });
    let result = call_op("arrow_field", &[arrow.clone(), json!("first")]);
    assert_eq!(result, json!(1));

    let result2 = call_op("arrow_field", &[arrow, json!("second")]);
    assert_eq!(result2, json!(2));
}

#[test]
fn exec_model_arrow_field_missing() {
    let result = call_op(
        "arrow_field",
        &[json!({ "__tag__": "Arrow", "first": 1 }), json!("second")],
    );
    assert_eq!(result, Value::Null);
}

// ===========================================================================
// Part 2: run through the Model trait — parity with ValueModel
// ===========================================================================

#[test]
fn exec_model_run_arr() {
    let term = Term::Arr { value: json!(5) };
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(5));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_const() {
    let term = Term::Const { value: json!(7) };
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(7));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_seq() {
    let term = Term::seq(
        Term::Arr { value: json!(1) },
        Term::op_native("inc", vec![]),
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(2));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_tag() {
    let term = Term::Tag {
        tag: "ok".to_string(),
        value: Box::new(Term::Arr { value: json!(42) }),
    };
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!({ "__tag__": "ok", "value": 42 }));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_record_proj() {
    let term = Term::seq(
        Term::Arr {
            value: json!({ "a": 1, "b": 2 }),
        },
        Term::RecordProj {
            record: Box::new(Term::op_native("id", vec![])),
            field: "a".to_string(),
        },
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(1));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_record_with() {
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
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!({ "a": 10, "b": 2 }));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_op_add() {
    let term = Term::op_native(
        "add",
        vec![Term::Arr { value: json!(1) }, Term::Arr { value: json!(2) }],
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(3));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_op_mul() {
    let term = Term::op_native(
        "mul",
        vec![Term::Arr { value: json!(3) }, Term::Arr { value: json!(4) }],
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(12));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_op_is_pos() {
    let term = Term::seq(
        Term::Arr { value: json!(3) },
        Term::op_native("is_pos", vec![]),
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(true));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_op_inc() {
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::op_native("inc", vec![]),
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(6));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_path() {
    let term = Term::Path {
        endpoints: (
            Box::new(Term::Arr { value: json!(1) }),
            Box::new(Term::Arr { value: json!(2) }),
        ),
        witness: Some(Box::new(Term::Arr { value: json!(3) })),
    };
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(
        result,
        json!({
            "__tag__": "Path",
            "from": 1,
            "to": 2,
            "witness": 3
        })
    );
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_equiv_bool() {
    let term = Term::Equiv {
        a: Box::new(Term::Arr { value: json!(1) }),
        b: Box::new(Term::Arr { value: json!(1) }),
        path: None,
    };
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(true));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_cell_id() {
    let term = Term::CellId {
        arrow: Box::new(Term::Arr { value: json!(7) }),
    };
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(
        result,
        json!({
            "__tag__": "Cell2",
            "__src__": 7,
            "__tgt__": 7,
            "value": 7
        })
    );
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_rewrite() {
    use handl_language::RewriteWitness;
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::Rewrite {
            source: Box::new(Term::Arr { value: json!(1) }),
            target: Box::new(Term::op_native("id", vec![])),
            witness: RewriteWitness::Named("x".to_string()),
        },
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(5));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_refine() {
    let term = Term::seq(
        Term::Arr { value: json!(5) },
        Term::Refine {
            sort: Box::new(Term::op_native("id", vec![])),
            predicate: Box::new(Term::op_native("is_pos", vec![])),
        },
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(5));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_cobind() {
    let term = Term::seq(
        Term::Arr { value: json!(99) },
        Term::Cobind {
            grade: Box::new(Term::op_native("id", vec![])),
            body: Box::new(Term::op_native("id", vec![])),
        },
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(99));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_iter() {
    let term = Term::Iter {
        fuel: Box::new(Term::Arr { value: json!(10) }),
        predicate: Box::new(Term::op_native("is_pos", vec![])),
        step: Box::new(Term::op_native("dec", vec![])),
        initial: Box::new(Term::Arr { value: json!(3) }),
    };
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(0));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_first() {
    let term = Term::seq(
        Term::Arr {
            value: json!({ "__pair__": [1, 2] }),
        },
        Term::first(Term::op_native("inc", vec![])),
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(
        result,
        json!({ "__pair__": [2, 2] })
    );
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_choice_left() {
    let term = Term::seq(
        Term::Arr {
            value: json!({ "__either__": "left", "value": 5 }),
        },
        Term::choice(
            Term::op_native("inc", vec![]),
            Term::op_native("dec", vec![]),
        ),
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(6));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_choice_right() {
    let term = Term::seq(
        Term::Arr {
            value: json!({ "__either__": "right", "value": 5 }),
        },
        Term::choice(
            Term::op_native("inc", vec![]),
            Term::op_native("dec", vec![]),
        ),
    );
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(4));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_app_tag() {
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
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!({ "__tag__": "ok", "value": 42 }));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_handle_number_add() {
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
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, number_value(3, &["Addable"]));
    assert_eq!(result, rust_run(&term, Value::Null));
}

// ===========================================================================
// Part 3: Comprehensive parity — exec model vs ValueModel vs Rust engine
// ===========================================================================

#[test]
fn exec_model_parity_with_value_model_and_rust() {
    // A batch of terms where the exec model, value_model.hnl, and the Rust
    // tree-walk engine all agree — confirming the exec model's `run` is a
    // faithful member of the model family on the overlapping tags.
    let cases: Vec<(Term, Value)> = vec![
        (Term::Arr { value: json!(5) }, json!(5)),
        (Term::Const { value: json!(7) }, json!(7)),
        (
            Term::seq(
                Term::Arr { value: json!(1) },
                Term::op_native("inc", vec![]),
            ),
            json!(2),
        ),
        (
            Term::op_native(
                "add",
                vec![Term::Arr { value: json!(1) }, Term::Arr { value: json!(2) }],
            ),
            json!(3),
        ),
        (
            Term::op_native(
                "mul",
                vec![Term::Arr { value: json!(3) }, Term::Arr { value: json!(4) }],
            ),
            json!(12),
        ),
        (
            Term::Tag {
                tag: "ok".to_string(),
                value: Box::new(Term::Arr { value: json!(42) }),
            },
            json!({ "__tag__": "ok", "value": 42 }),
        ),
        (
            Term::CellId {
                arrow: Box::new(Term::Arr { value: json!(7) }),
            },
            json!({
                "__tag__": "Cell2",
                "__src__": 7,
                "__tgt__": 7,
                "value": 7
            }),
        ),
        (
            Term::seq(
                Term::Arr { value: json!(5) },
                Term::Rewrite {
                    source: Box::new(Term::Arr { value: json!(1) }),
                    target: Box::new(Term::op_native("id", vec![])),
                    witness: handl_language::RewriteWitness::Named("w".to_string()),
                },
            ),
            json!(5),
        ),
    ];

    for (term, expected) in cases {
        let exec = exec_model_run(&term, Value::Null);
        let rust = rust_run(&term, Value::Null);
        let vm = value_model_run(&term, Value::Null);
        assert_eq!(exec, expected, "exec model mismatch for {:?}", term);
        assert_eq!(rust, expected, "rust engine mismatch for {:?}", term);
        assert_eq!(vm, expected, "value model mismatch for {:?}", term);
        assert_eq!(exec, rust, "exec/rust disagreement for {:?}", term);
        assert_eq!(exec, vm, "exec/value_model disagreement for {:?}", term);
    }
}

// ===========================================================================
// Part 4: Module loads with all structural ops defined
// ===========================================================================

#[test]
fn exec_model_module_loads_all_ops() {
    // Verify that the module loads and all structural op functions are
    // resolvable — even the spec-only ones that require VM primitives.
    // (They parse and are defined; calling them would fail at runtime.)
    let module = exec_model_module();
    for &name in EXEC_MODEL_FUNCS {
        assert!(
            module.function(name).is_some(),
            "exec model module should export '{}'",
            name
        );
    }
}

#[test]
fn exec_model_adapter_is_model() {
    // The HandlModelAdapter wrapping the exec model satisfies the Model
    // trait and can run terms.
    let term = Term::Arr { value: json!(42) };
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(42));
}
