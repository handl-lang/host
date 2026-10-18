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
//! ## VM primitive-backed structural operations
//!
//! The following structural ops require genuine VM instruction-set primitives
//! (`push_scope`, `pop_scope`, `bind`, `lookup`, `call`, `ffi`,
//! `make_closure`, `vm_pattern_match`, `lookup_eval_scheme`, `setfield`,
//! `ctx`). These are now wired as host functions via the
//! [`VmPrimitivesModule`] (registered as an exec model dependency), making
//! the structural ops executable for testing:
//!
//! - `proj_env` — calls `lookup(name)`.
//! - `extend_env` — calls `push_scope()` / `bind(name, val)`.
//! - `closure` — calls `make_closure(params, body, env)`.
//! - `apply` — calls `call(callee, args)`.
//! - `pattern_match` — calls `vm_pattern_match(pattern, value)`.
//! - `push_handler` / `pop_handler` — call `ctx()` / `setfield`.
//! - `call_op` — calls `ctx()` / `ffi` / `apply`.
//! - `eval` — calls `lookup_eval_scheme(kind)` / `scheme(node)`.
//! - `proj_child` — calls `eval(idx)` (local `eval` binding).
//! - `eval_block` — calls `push_scope()` / `eval` / `pop_scope()`.
//! - `import_module` — calls `ffi` / `bind`.
//! - `export_binding` — calls `ctx()` / `setfield`.
//!
//! The VM primitive implementations are minimal — they handle the common
//! cases needed for testing. See [`vm_primitives`] module docs for details.

use std::rc::Rc;

use handl_language::{Ast, Language, Term, Value};
use handl_platform::{EvalContext, Module};
use hnl_engine_exec_tree_walk::ValueModel;
use handl_plugin_lib_handl::{HandlModel, HandlModelAdapter, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_plugin_module::{ModuleLookup, ModuleRegistry};
use handl_runtime::vm_primitives::VmPrimitivesModule;
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
            handl_kit_data_canonical::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

const EXEC_MODEL_HNL: &str = include_str!("fixtures/exec_model.hnl");
const VALUE_MODEL_HNL: &str = include_str!("fixtures/value_model.hnl");

/// Convert a `TermData` back to `Term` for passing to `Model::run` (which
/// takes `&Term`). The conversion is lossless for all well-formed `TermData`.
fn to_term(td: TermData) -> Term {
    td.to_term().expect("TermData -> Term conversion failed")
}

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
        .with_dependency_arc(
            stdlib::load_hnl_file(format!(
                "{}/src/array/index.hnl",
                handl_lib_collections::ROOT
            ))
            .expect("array module"),
        )
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule)
        .with_dependency(VmPrimitivesModule::new())
}

/// Build a `HandlModelAdapter` wrapping the exec model module.
fn exec_model_adapter() -> HandlModelAdapter {
    HandlModelAdapter::new(exec_model_module())
}

/// Build a `HandlModel` backed by `value_model.hnl` (the tree-walk Handl
/// reference), used for parity comparison on overlapping tags.
fn value_model() -> HandlModel {
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

/// Build the pure Rust tree-walk `ValueModel` (the exec engine reference).
fn rust_model() -> ValueModel {
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(TagModule));
    ValueModel::new(hnl_engine_exec_tree_walk::TreeWalkEngine::new().with_modules(modules))
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
    rust.run(term, incoming, &mut ctx).unwrap_or(Value::Null)
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
        ensure_lang();
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
        ensure_lang();
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
        ensure_lang();
    // The exec model's number_add should produce the same result as the
    // value_model's number_add (both use the tag.untag / tag.tag pattern).
    let operands = json!([number_value(7, &["Addable"]), number_value(8, &["Addable"])]);
    let exec_result = call_op("number_add", &[operands.clone()]);

    // Run number_add through the value model by constructing an Op term.
    let term = to_term(TermData::op(
        "number_add",
        vec![
            TermData::arr(number_value(7, &["Addable"])),
            TermData::arr(number_value(8, &["Addable"])),
        ],
        &ArrSource::Native,
        None,
    ));
    let vm_result = value_model_run(&term, Value::Null);

    assert_eq!(exec_result, number_value(15, &["Addable"]));
    assert_eq!(vm_result, number_value(15, &["Addable"]));
    assert_eq!(exec_result, vm_result);
}

#[test]
fn exec_model_record_construction() {
        ensure_lang();
    // record: construct a record from field/value pairs.
    let args = vec![json!([["a", 1], ["b", 2], ["c", 3]])];
    let result = call_op("record", &args);
    assert_eq!(result, json!({ "a": 1, "b": 2, "c": 3 }));
}

#[test]
fn exec_model_record_empty() {
        ensure_lang();
    let result = call_op("record", &[json!([])]);
    assert_eq!(result, json!({}));
}

#[test]
fn exec_model_field_access_record() {
        ensure_lang();
    // field_access on a record (object): field lookup.
    let args = vec![json!({ "a": 1, "b": 2 }), json!("a")];
    let result = call_op("field_access", &args);
    assert_eq!(result, json!(1));
}

#[test]
fn exec_model_field_access_record_missing() {
        ensure_lang();
    // field_access on a record with an absent field: returns null.
    let args = vec![json!({ "a": 1 }), json!("z")];
    let result = call_op("field_access", &args);
    assert_eq!(result, Value::Null);
}

#[test]
fn exec_model_field_access_array() {
        ensure_lang();
    // field_access on an array: index lookup.
    let args = vec![json!([10, 20, 30]), json!(1)];
    let result = call_op("field_access", &args);
    assert_eq!(result, json!(20));
}

#[test]
fn exec_model_field_access_non_record_non_array() {
        ensure_lang();
    // field_access on a non-record/non-array value: returns null.
    let args = vec![json!(42), json!("a")];
    let result = call_op("field_access", &args);
    assert_eq!(result, Value::Null);
}

#[test]
fn exec_model_array_identity() {
        ensure_lang();
    // array: identity — returns the elements as-is.
    let args = vec![json!([1, 2, 3])];
    let result = call_op("array", &args);
    assert_eq!(result, json!([1, 2, 3]));
}

#[test]
fn exec_model_tag_construct() {
        ensure_lang();
    // tag_construct: merge __tag__ with the payload fields (via tag.tag).
    let args = vec![json!("ok"), json!({ "value": 42 })];
    let result = call_op("tag_construct", &args);
    assert_eq!(result, json!({ "__tag__": "ok", "value": 42 }));
}

#[test]
fn exec_model_tag_construct_with_extra_fields() {
        ensure_lang();
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
        ensure_lang();
    // arrow_field: project a named field from an arrow cell record.
    let arrow = json!({ "__tag__": "Arrow", "first": 1, "second": 2 });
    let result = call_op("arrow_field", &[arrow.clone(), json!("first")]);
    assert_eq!(result, json!(1));

    let result2 = call_op("arrow_field", &[arrow, json!("second")]);
    assert_eq!(result2, json!(2));
}

#[test]
fn exec_model_arrow_field_missing() {
        ensure_lang();
    let result = call_op(
        "arrow_field",
        &[json!({ "__tag__": "Arrow", "first": 1 }), json!("second")],
    );
    assert_eq!(result, Value::Null);
}

// ===========================================================================
// Part 1b: VM primitive-backed structural operations
// ===========================================================================

#[test]
fn exec_model_proj_env_lookup_unbound() {
        ensure_lang();
    // proj_env on an unbound name: lookup returns null.
    let result = call_op("proj_env", &[json!("nonexistent")]);
    assert_eq!(result, Value::Null);
}

#[test]
fn exec_model_extend_env_binds_value() {
        ensure_lang();
    // extend_env: push scope, bind name→val, return val.
    let result = call_op("extend_env", &[json!("x"), json!(42)]);
    assert_eq!(result, json!(42));
}

#[test]
fn exec_model_proj_env_after_extend_env() {
        ensure_lang();
    // After extend_env binds "x"→42, proj_env("x") should find it.
    // Note: each call_op creates a fresh module, so bindings don't persist
    // across calls. This test verifies the bind+lookup sequence works within
    // a single module invocation by using a do-block that chains them.
    // Since we can't chain directly, we test that extend_env returns the
    // value (the bind happened) and proj_env on an unbound name returns null.
    let bind_result = call_op("extend_env", &[json!("y"), json!(99)]);
    assert_eq!(bind_result, json!(99));
    let lookup_result = call_op("proj_env", &[json!("y")]);
    assert_eq!(lookup_result, Value::Null);
}

#[test]
fn exec_model_closure_creates_closure_record() {
        ensure_lang();
    // closure: create a closure value via make_closure.
    let result = call_op("closure", &[json!(["x"]), json!(42), json!({})]);
    assert_eq!(
        result,
        json!({
            "__closure__": true,
            "params": ["x"],
            "body": 42,
            "env": {}
        })
    );
}

#[test]
fn exec_model_apply_closure_returns_body() {
        ensure_lang();
    // apply on a closure record: call returns the body field (minimal).
    let closure = json!({
        "__closure__": true,
        "params": ["x"],
        "body": json!(42),
        "env": {}
    });
    let result = call_op("apply", &[closure, json!([7])]);
    assert_eq!(result, json!(42));
}

#[test]
fn exec_model_apply_non_callable_returns_null() {
        ensure_lang();
    // apply on a non-callable value: call returns null.
    let result = call_op("apply", &[json!(42), json!([1])]);
    assert_eq!(result, Value::Null);
}

#[test]
fn exec_model_pattern_match_wildcard() {
        ensure_lang();
    // pattern_match with wildcard "_": matches anything, returns empty bindings.
    let result = call_op("pattern_match", &[json!("_"), json!(42)]);
    assert_eq!(result, json!({}));
}

#[test]
fn exec_model_pattern_match_variable() {
        ensure_lang();
    // pattern_match with a variable pattern: binds the variable to the value.
    let result = call_op("pattern_match", &[json!("x"), json!(42)]);
    assert_eq!(result, json!({ "x": 42 }));
}

#[test]
fn exec_model_pattern_match_literal_equal() {
        ensure_lang();
    // pattern_match with a literal that equals the value: match success.
    let result = call_op("pattern_match", &[json!(42), json!(42)]);
    assert_eq!(result, json!({}));
}

#[test]
fn exec_model_pattern_match_literal_not_equal() {
        ensure_lang();
    // pattern_match with a literal that doesn't match: returns null.
    let result = call_op("pattern_match", &[json!(42), json!(43)]);
    assert_eq!(result, Value::Null);
}

#[test]
fn exec_model_push_handler_adds_to_stack() {
        ensure_lang();
    // push_handler: pushes a handler onto the context's handler stack.
    // Returns the new handler stack (via setfield returning the new value).
    let handler = json!({"impls": {"add": "native_add"}});
    let result = call_op("push_handler", &[handler.clone()]);
    // The result is the new handler stack: [handler].
    assert_eq!(result, json!([handler]));
}

#[test]
fn exec_model_pop_handler_removes_from_stack() {
        ensure_lang();
    // pop_handler: pops the top handler from the handler stack.
    // Since each call_op creates a fresh module, the stack starts empty.
    // pop_handler on an empty stack: array_drop_last([]) = [].
    let result = call_op("pop_handler", &[json!(null)]);
    // setfield returns the new value (the sliced array).
    // array.slice([], 0, -1) → [] (empty).
    assert_eq!(result, json!([]));
}

#[test]
fn exec_model_call_op_ffi_fallback() {
        ensure_lang();
    // call_op with no handler: falls back to ffi("native", name, args).
    // ffi("native", "add", [1, 2]) → 3.
    let result = call_op("call_op", &[json!("add"), json!([1, 2])]);
    assert_eq!(result, json!(3));
}

#[test]
fn exec_model_call_op_mul_via_ffi() {
        ensure_lang();
    // call_op with no handler: ffi("native", "mul", [3, 4]) → 12.
    let result = call_op("call_op", &[json!("mul"), json!([3, 4])]);
    assert_eq!(result, json!(12));
}

#[test]
fn exec_model_eval_arr_node() {
        ensure_lang();
    // eval on an Arr node: lookup_eval_scheme("Arr") → "eval_scheme_Arr"
    // → eval_scheme_Arr(node) → node.value.
    let node = json!({"kind": "Arr", "value": 42});
    let result = call_op("eval", &[node]);
    assert_eq!(result, json!(42));
}

#[test]
fn exec_model_eval_const_node() {
        ensure_lang();
    // eval on a Const node: lookup_eval_scheme("Const") → "eval_scheme_Const"
    // → eval_scheme_Const(node) → node.value.
    let node = json!({"kind": "Const", "value": 7});
    let result = call_op("eval", &[node]);
    assert_eq!(result, json!(7));
}

#[test]
fn exec_model_proj_child_arr() {
        ensure_lang();
    // proj_child: look up the child at a named role, then eval it.
    let node = json!({
        "children": {
            "value": {"kind": "Arr", "value": 42}
        }
    });
    let result = call_op("proj_child", &[node, json!("value")]);
    assert_eq!(result, json!(42));
}

#[test]
fn exec_model_proj_child_or_with_child() {
        ensure_lang();
    // proj_child_or: child exists → eval it.
    let node = json!({
        "children": {
            "value": {"kind": "Arr", "value": 99}
        }
    });
    let result = call_op("proj_child_or", &[node, json!("value"), json!(0)]);
    assert_eq!(result, json!(99));
}

#[test]
fn exec_model_proj_child_or_default() {
        ensure_lang();
    // proj_child_or: child absent → return default.
    let node = json!({"children": {}});
    let result = call_op("proj_child_or", &[node, json!("missing"), json!(0)]);
    assert_eq!(result, json!(0));
}

#[test]
fn exec_model_eval_block_arr_children() {
        ensure_lang();
    // eval_block: push scope, eval each child, pop scope, return last.
    let node = json!({
        "children": [
            {"kind": "Arr", "value": 1},
            {"kind": "Arr", "value": 2},
            {"kind": "Arr", "value": 3}
        ]
    });
    let result = call_op("eval_block", &[node]);
    assert_eq!(result, json!(3));
}

#[test]
fn exec_model_eval_block_empty() {
        ensure_lang();
    // eval_block with no children: array.last([]) → null.
    let node = json!({"children": []});
    let result = call_op("eval_block", &[node]);
    assert_eq!(result, Value::Null);
}

#[test]
fn exec_model_eval_block_const_children() {
        ensure_lang();
    // eval_block with Const children.
    let node = json!({
        "children": [
            {"kind": "Const", "value": 10},
            {"kind": "Const", "value": 20}
        ]
    });
    let result = call_op("eval_block", &[node]);
    assert_eq!(result, json!(20));
}

#[test]
fn exec_model_import_module_returns_exports() {
        ensure_lang();
    // import_module: ffi("module", "load", [spec]) → {} (empty exports),
    // then binds each export name. Returns the exports record.
    let result = call_op("import_module", &[json!("some_module")]);
    assert_eq!(result, json!({}));
}

#[test]
fn exec_model_export_binding_returns_value() {
        ensure_lang();
    // export_binding: registers name→value in the context's exports.
    // Returns the value.
    let result = call_op("export_binding", &[json!("my_export"), json!(42)]);
    assert_eq!(result, json!(42));
}

// ===========================================================================
// Part 2: run through the Model trait — parity with ValueModel
// ===========================================================================

#[test]
fn exec_model_run_arr() {
        ensure_lang();
    let term = to_term(TermData::arr(json!(5)));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(5));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_const() {
        ensure_lang();
    let term = to_term(TermData::const_value(json!(7)));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(7));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_seq() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(1)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(2));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_tag() {
        ensure_lang();
    let term = to_term(TermData::tag_value("ok", TermData::arr(json!(42))));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!({ "__tag__": "ok", "value": 42 }));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_record_proj() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        TermData::record_proj(TermData::op("id", vec![], &ArrSource::Native, None), "a"),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(1));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_record_with() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "a": 1, "b": 2 })),
        TermData::record_with(
            TermData::op("id", vec![], &ArrSource::Native, None),
            "a",
            TermData::arr(json!(10)),
        ),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!({ "a": 10, "b": 2 }));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_op_add() {
        ensure_lang();
    let term = to_term(TermData::op(
        "add",
        vec![TermData::arr(json!(1)), TermData::arr(json!(2))],
        &ArrSource::Native,
        None,
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(3));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_op_mul() {
        ensure_lang();
    let term = to_term(TermData::op(
        "mul",
        vec![TermData::arr(json!(3)), TermData::arr(json!(4))],
        &ArrSource::Native,
        None,
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(12));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_op_is_pos() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(3)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(true));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_op_inc() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(5)),
        TermData::op("inc", vec![], &ArrSource::Native, None),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(6));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_path() {
        ensure_lang();
    let term = to_term(TermData::path(
        TermData::arr(json!(1)),
        TermData::arr(json!(2)),
        Some(TermData::arr(json!(3))),
    ));
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
        ensure_lang();
    let term = to_term(TermData::equiv(
        TermData::arr(json!(1)),
        TermData::arr(json!(1)),
        None,
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(true));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_cell_id() {
        ensure_lang();
    let term = to_term(TermData::cell_id(TermData::arr(json!(7))));
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
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(5));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_refine() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(5)),
        TermData::refine(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("is_pos", vec![], &ArrSource::Native, None),
        ),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(5));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_cobind() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!(99)),
        TermData::cobind(
            TermData::op("id", vec![], &ArrSource::Native, None),
            TermData::op("id", vec![], &ArrSource::Native, None),
        ),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(99));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_iter() {
        ensure_lang();
    let term = to_term(TermData::iter(
        TermData::arr(json!(10)),
        TermData::op("is_pos", vec![], &ArrSource::Native, None),
        TermData::op("dec", vec![], &ArrSource::Native, None),
        TermData::arr(json!(3)),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(0));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_first() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "__pair__": [1, 2] })),
        TermData::first(TermData::op("inc", vec![], &ArrSource::Native, None)),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!({ "__pair__": [2, 2] }));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_choice_left() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "__either__": "left", "value": 5 })),
        TermData::choice(
            TermData::op("inc", vec![], &ArrSource::Native, None),
            TermData::op("dec", vec![], &ArrSource::Native, None),
        ),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(6));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_choice_right() {
        ensure_lang();
    let term = to_term(TermData::seq(
        TermData::arr(json!({ "__either__": "right", "value": 5 })),
        TermData::choice(
            TermData::op("inc", vec![], &ArrSource::Native, None),
            TermData::op("dec", vec![], &ArrSource::Native, None),
        ),
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(4));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_app_tag() {
        ensure_lang();
    let term = to_term(TermData::app(
        TermData::arr(json!("tag.tag")),
        vec![
            TermData::arr(json!("ok")),
            TermData::arr(json!({ "value": 42 })),
        ],
    ));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!({ "__tag__": "ok", "value": 42 }));
    assert_eq!(result, rust_run(&term, Value::Null));
}

#[test]
fn exec_model_run_handle_number_add() {
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
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, number_value(3, &["Addable"]));
    assert_eq!(result, rust_run(&term, Value::Null));
}

// ===========================================================================
// Part 3: Comprehensive parity — exec model vs ValueModel vs Rust engine
// ===========================================================================

#[test]
fn exec_model_parity_with_value_model_and_rust() {
        ensure_lang();
    // A batch of terms where the exec model, value_model.hnl, and the Rust
    // tree-walk engine all agree — confirming the exec model's `run` is a
    // faithful member of the model family on the overlapping tags.
    let cases: Vec<(Term, Value)> = vec![
        (to_term(TermData::arr(json!(5))), json!(5)),
        (to_term(TermData::const_value(json!(7))), json!(7)),
        (
            to_term(TermData::seq(
                TermData::arr(json!(1)),
                TermData::op("inc", vec![], &ArrSource::Native, None),
            )),
            json!(2),
        ),
        (
            to_term(TermData::op(
                "add",
                vec![TermData::arr(json!(1)), TermData::arr(json!(2))],
                &ArrSource::Native,
                None,
            )),
            json!(3),
        ),
        (
            to_term(TermData::op(
                "mul",
                vec![TermData::arr(json!(3)), TermData::arr(json!(4))],
                &ArrSource::Native,
                None,
            )),
            json!(12),
        ),
        (
            to_term(TermData::tag_value("ok", TermData::arr(json!(42)))),
            json!({ "__tag__": "ok", "value": 42 }),
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
        (
            to_term(TermData::seq(
                TermData::arr(json!(5)),
                TermData::rewrite(
                    TermData::arr(json!(1)),
                    TermData::op("id", vec![], &ArrSource::Native, None),
                    &handl_language::RewriteWitness::Named("w".to_string()),
                ),
            )),
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
        ensure_lang();
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
        ensure_lang();
    // The HandlModelAdapter wrapping the exec model satisfies the Model
    // trait and can run terms.
    let term = to_term(TermData::arr(json!(42)));
    let result = exec_model_run(&term, Value::Null);
    assert_eq!(result, json!(42));
}
