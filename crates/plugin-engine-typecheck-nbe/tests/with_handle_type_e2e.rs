//! End-to-end integration test for `Term::Handle` in `TypeModel`.
//!
//! Demonstrates that a `Term::Handle` pushes a handler onto the type-level
//! `ModelCtx` handler stack and that the graded `Term::Op` body dispatches to
//! that handler, returning the `Number` type.

use handl_language::{ArrSource, Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_typecheck_nbe::{NbeTypecheckEngine, TypeModel};
use plugin_engine::{Model, ModelCtx};
use serde_json::json;

/// The canonical `type.number` value minted by `handl.canonical`.
fn type_number() -> Value {
    json!({"__tag__": "type.number", "__registry__": "handl.canonical"})
}

/// A `Number` literal value (no grades needed on the arguments themselves).
fn number_arg(value: f64) -> Value {
    json!({
        "__tag__": "Number",
        "__registry__": "handl.canonical",
        "value": value
    })
}

/// An incoming `Number` value carrying `Addable` for the graded op.
fn number_value(grades: &[&str]) -> Value {
    json!({
        "__tag__": "Number",
        "__registry__": "handl.canonical",
        "grades": grades,
        "value": 0.0
    })
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

#[test]
fn with_handle_returns_number_type() {
    // 1. Build a `handl_language::Language` from the canonical kit.
    let mut lang = handl_kit_data_canonical::language_from_yaml().unwrap();

    // 2. Register the `add` op and `Addable` handler.
    lang.register_op("add", add_op_decl());
    lang.register_handler("Addable", add_handler());

    // 3. Construct `handle add(Number(1), Number(2)) with "" = AddableHandler`.
    let body = Term::Op {
        dim: Box::new(Term::Const { value: json!(1) }),
        name: "add".to_string(),
        args: vec![
            Term::Arr {
                dim: Box::new(Term::Const { value: json!(1) }),
                value: number_arg(1.0),
            },
            Term::Arr {
                dim: Box::new(Term::Const { value: json!(1) }),
                value: number_arg(2.0),
            },
        ],
        source: ArrSource::Native,
        lit: None,
    };
    let with_term = Term::Handle {
        body: Box::new(body),
        target: "".to_string(),
        impl_: Box::new(Term::Arr {
            dim: Box::new(Term::Const { value: json!(1) }),
            value: add_handler(),
        }),
    };

    // 4. Set up a `ModelCtx` with an empty `handler_stack`.
    let ast = Ast::new();
    let incoming = number_value(&["Addable"]);
    let mut ctx = model_ctx(&lang, &ast, vec![]);

    // 5. Run `TypeModel` and assert the result is the `number` type.
    let result = TypeModel(NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new())))
        .run(&with_term, incoming, &mut ctx)
        .unwrap();

    assert_eq!(result, type_number());
}
