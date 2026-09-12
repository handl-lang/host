//! End-to-end integration test for the `with`/`handle` surface syntax
//! combined with `ValueModel` and grade-based handler dispatch.
//!
//! The test demonstrates that a `TermData::handle` pushes a handler onto the
//! runtime handler stack, and that a graded `TermData::op` inside the body
//! dispatches to that handler. The canonical `Language` is built from the
//! data-driven canonical kit; the `add` op and the `Addable` handler are
//! registered directly on `lang.ops` and `lang.handlers_by_grade`.

use handl_language::{ArrSource, tag_value};
use handl_language::{Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::{TreeWalkEngine, ValueModel};
use plugin_engine::{Model, ModelCtx, TermData};
use serde_json::json;

/// Convert `TermData` back to `Term` for engine APIs that require `Term`.
fn td(t: TermData) -> Term {
    t.to_term().expect("TermData -> Term conversion")
}

/// A canonical `Number` value carrying the `Addable` grade.
fn number_value(grades: &[&str], value: f64) -> Value {
    let payload = serde_json::json!({
        "grades": grades,
        "value": value
    });
    tag_value("Number", &payload).expect("Number is a valid tag value")
}

/// The `add` op declaration, graded `Addable`.
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

/// A `Handler` value whose `add` implementation is native `number_add`.
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

fn model_ctx<'a>(
    lang: &'a Language,
    ast: &'a Ast,
    node: &'a plugin_engine::AstNode,
    handler_stack: Vec<Value>,
) -> ModelCtx<'a> {
    let lang_view: &dyn plugin_engine::LanguageView = lang;
    let ast_view: &dyn plugin_engine::AstView = ast;
    ModelCtx {
        ast: ast_view,
        lang: lang_view,
        current_node: node,
        node_ref: 0,
        handler_stack,
    }
}

#[test]
fn with_handle_pipeline_with_canonical_language() {
    // 1. Build a `Language` from the canonical data-driven kit.
    let mut lang = handl_kit_data_canonical::language_from_yaml().unwrap();

    // 2. Register the graded `add` op and the `Addable` handler on the
    //    `Language`. `handlers_by_grade` is the dispatch table used by
    //    `ValueModel` when no handler is present on the runtime stack.
    lang.ops.insert("add".to_string(), add_op_decl());
    lang.handlers_by_grade
        .insert("Addable".to_string(), add_handler());

    // 3. Manually construct the desugared form of:
    //      with NumberHandler do add(1, 2)
    //    `impl_` is the handler value; `body` is the graded `add` call.
    let left = TermData::arr(number_value(&["Addable"], 1.0));
    let right = TermData::arr(number_value(&["Addable"], 2.0));
    let body = TermData::op(
        "add",
        vec![left, right],
        &ArrSource::Native,
        None,
    );
    let with_term = td(TermData::handle(
        body,
        "",
        TermData::arr(add_handler()),
    ));

    // 4. Evaluate with `ValueModel`; incoming value carries `Addable`.
    let ast = Ast::new();
    let current_node = plugin_engine::AstNode;
    let incoming = number_value(&["Addable"], 0.0);
    let mut ctx = model_ctx(&lang, &ast, &current_node, vec![]);

    let result = ValueModel::new(TreeWalkEngine::new())
        .run(&with_term, incoming, &mut ctx)
        .unwrap();

    // 5. Assert the result is a `Number` with value `3.0`.
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
