//! End-to-end integration test for the full `sort`/`grade`/`op`/`handler`
//! pipeline using canonical language data and `ValueModel`.
//!
//! The `sort` and `grade` are encoded in the value shape (`__tag__: Number`,
//! `grades: ["Addable"]`); the `op` and `handler` are registered on the
//! canonical `Language` via `lang.ops` and `lang.handlers_by_grade` and driven
//! through `handl_plugin_engine_exec_tree_walk::ValueModel`.

use handl_language::{ArrSource, Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::{TreeWalkEngine, ValueModel};
use plugin_engine::{Model, ModelCtx, TermData};
use serde_json::json;

/// Convert `TermData` back to `Term` for engine APIs that require `Term`.
fn td(t: TermData) -> Term {
    t.to_term().expect("TermData -> Term conversion")
}

/// A `Number` value carrying the `Addable` grade.
fn number_value(grades: &[&str], value: f64) -> Value {
    json!({
        "__tag__": "Number",
        "__registry__": "handl.canonical",
        "grades": grades,
        "value": value
    })
}

/// A canonical-style `op` declaration for `add`.
fn add_op_decl() -> Value {
    json!({
        "__tag__": "op",
        "__registry__": "handl.canonical",
        "name": "add",
        "inputs": ["Number", "Number"],
        "output": "Number",
        "grade": "Addable"
    })
}

/// A `Handler` value whose `add` implementation returns `expected`.
fn add_handler() -> Value {
    json!({
        "__tag__": "Handler",
        "__registry__": "handl.canonical",
        "grades": ["Addable"],
        "impls": {
            "add": serde_json::to_value(td(TermData::op("number_add", vec![], &ArrSource::Native, None))).unwrap()
        }
    })
}

fn model_ctx<'a>(lang: &'a Language, ast: &'a Ast, handler_stack: Vec<Value>) -> ModelCtx<'a> {
    let lang_view: &dyn plugin_engine::LanguageView = lang;
    let ast_view: &dyn plugin_engine::AstView = ast;
    ModelCtx {
        ast: ast_view,
        lang: lang_view,
        current_node: &plugin_engine::AstNode,
        node_ref: 0,
        handler_stack,
    }
}

#[test]
fn sort_grade_op_handler_pipeline_with_canonical_language() {
    // 1. Build a Language from the canonical data.
    let mut lang = handl_kit_data_canonical::language_from_yaml().unwrap();

    // 2. Register the `sort` (Number value), `grade` (Addable key), `op`,
    //    and `handler` on the Language.
    let left = number_value(&["Addable"], 1.0);
    let right = number_value(&["Addable"], 2.0);
    let expected = number_value(&["Addable"], 3.0);

    lang.ops.insert("add".to_string(), add_op_decl());
    lang.handlers_by_grade
        .insert("Addable".to_string(), add_handler());

    // Sanity-check the Language registration API.
    let decl = lang
        .op_decl("add")
        .expect("add op_decl should be registered");
    assert_eq!(decl.get("grade").and_then(Value::as_str), Some("Addable"));
    assert!(lang.handler_for_grade("Addable").is_some());

    // 3. Incoming `Number` value carrying the `Addable` grade.
    let incoming = number_value(&["Addable"], 0.0);

    // 4. Construct an `add` term with two `Number` arguments.
    let add_term = td(TermData::op(
        "add",
        vec![TermData::arr(left), TermData::arr(right)],
        &ArrSource::Native,
        None,
    ));

    // 5. Create a `ModelCtx` with the canonical `Language`, a minimal `Ast`,
    //    and an empty `handler_stack`.
    let ast = Ast::new();
    let mut ctx = model_ctx(&lang, &ast, vec![]);

    // 6. Run `ValueModel` on the `Term`.
    let result = ValueModel::new(TreeWalkEngine::new())
        .run(&add_term, incoming, &mut ctx)
        .unwrap();

    // 7. Assert the result is a `Number` with the expected sum.
    assert_eq!(result, expected);
    assert_eq!(
        result.get("__tag__").and_then(Value::as_str),
        Some("Number")
    );
    assert_eq!(result.get("value").and_then(Value::as_f64), Some(3.0));
    assert_eq!(
        result
            .get("grades")
            .and_then(Value::as_array)
            .map(|a| a.len()),
        Some(1)
    );
}
