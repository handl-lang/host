//! Grade-based `Op` dispatch tests for `TypeModel`.
//!
//! Mirrors the value-level `grade_dispatch` tests but asserts the type-level
//! result is the `Number` type.

use handl_language::{ArrSource, Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_typecheck_nbe::{NbeTypecheckEngine, TypeModel};
use plugin_engine::{Model, ModelCtx};
use serde_json::json;

/// The canonical `type.number` value minted by `handl.canonical`.
fn type_number() -> Value {
    json!({"__tag__": "type.number", "__registry__": "handl.canonical"})
}

/// An incoming `Number` value carrying `grades` for dispatch checks.
fn number_value(grades: &[&str]) -> Value {
    json!({
        "__tag__": "Number",
        "__registry__": "handl.canonical",
        "grades": grades,
        "value": 0.0
    })
}

/// An `op` declaration for `add` requiring the `Addable` grade.
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

/// An `Addable` `Handler` whose `add` implementation returns the `Number` type.
fn add_handler() -> Value {
    json!({
        "__tag__": "Handler",
        "__registry__": "handl.canonical",
        "grades": ["Addable"],
        "impls": {
            "add": serde_json::to_value(Term::const_value(type_number())).unwrap()
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

fn add_term() -> Term {
    Term::Op {
        name: "add".to_string(),
        args: vec![
            Term::Arr { value: json!(1.0) },
            Term::Arr { value: json!(2.0) },
        ],
        source: ArrSource::Native,
        lit: None,
    }
}

#[test]
fn grade_type_dispatch_returns_number_type() {
    let mut lang = handl_kit_data_canonical::language_from_yaml().unwrap();
    lang.register_op("add", add_op_decl());
    lang.register_handler("Addable", add_handler());

    let ast = Ast::new();
    let incoming = number_value(&["Addable"]);
    let mut ctx = model_ctx(&lang, &ast, vec![]);

    let result = TypeModel(NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new())))
        .run(&add_term(), incoming, &mut ctx)
        .unwrap();

    assert_eq!(result, type_number());
}
