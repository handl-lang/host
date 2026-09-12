//! Grade-based `Op` dispatch tests (ADR-0098/0100).
//!
//! Constructs `op` declarations and `Handler` values directly in Rust and
//! drives them through `ValueModel::dispatch_op`.

use handl_language::{ArrSource, Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::{TreeWalkEngine, ValueModel};
use plugin_engine::{Model, ModelCtx, TermData};
use serde_json::json;

/// Convert `TermData` back to `Term` for engine APIs that require `Term`.
fn td(t: TermData) -> Term {
    t.to_term().expect("TermData -> Term conversion")
}

/// Build a native `op` `TermData` with no literal payload.
fn op(name: &str, args: Vec<TermData>) -> TermData {
    TermData::op(name, args, &ArrSource::Native, None)
}

fn number_value(grades: &[&str]) -> Value {
    json!({
        "__tag__": "Number",
        "__registry__": "test",
        "grades": grades
    })
}

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

fn add_handler(return_value: Value) -> Value {
    json!({
        "__tag__": "Handler",
        "__registry__": "handl.canonical",
        "grades": ["Addable"],
        "impls": {
            "add": serde_json::to_value(td(TermData::arr(return_value))).unwrap()
        }
    })
}

fn add_handler_value(value: f64) -> Value {
    add_handler(json!(value))
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

fn add_term() -> Term {
    td(op(
        "add",
        vec![
            TermData::arr(json!(1.0)),
            TermData::arr(json!(2.0)),
        ],
    ))
}

fn setup() -> (Language, Ast, Value) {
    let mut lang = Language::new();
    lang.ops.insert("add".to_string(), add_op_decl());
    lang.handlers
        .insert("Addable".to_string(), add_handler_value(42.0));

    let ast = Ast::new();
    (lang, ast, number_value(&["Addable"]))
}

#[test]
fn grade_dispatches_to_language_handler() {
    let (lang, ast, incoming) = setup();
    let current_node = plugin_engine::AstNode;
    let mut ctx = model_ctx(&lang, &ast, &current_node, vec![]);

    let result = ValueModel::new(TreeWalkEngine::new())
        .run(&add_term(), incoming, &mut ctx)
        .unwrap();
    assert_eq!(result, json!(42.0));
}

#[test]
fn missing_grade_returns_not_implemented() {
    let (lang, ast, _incoming) = setup();
    let current_node = plugin_engine::AstNode;
    let incoming = number_value(&["Multiplicable"]);
    let mut ctx = model_ctx(&lang, &ast, &current_node, vec![]);

    let result = ValueModel::new(TreeWalkEngine::new()).run(&add_term(), incoming, &mut ctx);
    assert_eq!(result, Err(plugin_engine::LangError::NotImplemented));
}

#[test]
fn no_handler_falls_back_to_native() {
    let (mut lang, ast, incoming) = setup();
    lang.handlers.clear();
    let current_node = plugin_engine::AstNode;
    let mut ctx = model_ctx(&lang, &ast, &current_node, vec![]);

    let result = ValueModel::new(TreeWalkEngine::new())
        .run(&add_term(), incoming, &mut ctx)
        .unwrap();
    assert_eq!(result, json!(3.0));
}

#[test]
fn handler_stack_overrides_language_handler() {
    let (lang, ast, incoming) = setup();
    let current_node = plugin_engine::AstNode;
    let stack_handler = add_handler_value(100.0);
    let mut ctx = model_ctx(&lang, &ast, &current_node, vec![stack_handler]);

    let result = ValueModel::new(TreeWalkEngine::new())
        .run(&add_term(), incoming, &mut ctx)
        .unwrap();
    assert_eq!(result, json!(100.0));
}
