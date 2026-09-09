//! Path, equivalence, and transport semantics for `ValueModel` (ADR-0100).

use handl_language::{Ast, Language, Term};
use handl_plugin_engine_exec_tree_walk::{TreeWalkEngine, ValueModel};
use plugin_engine::{Model, ModelCtx};
use serde_json::json;

fn model_ctx<'a>(
    lang: &'a Language,
    ast: &'a Ast,
    current_node: &'a plugin_engine::AstNode,
) -> ModelCtx<'a> {
    let lang_view: &dyn plugin_engine::LanguageView = lang;
    let ast_view: &dyn plugin_engine::AstView = ast;
    ModelCtx {
        ast: ast_view,
        lang: lang_view,
        current_node,
        node_ref: 0,
        handler_stack: vec![],
    }
}

#[test]
fn path_value() {
    let lang = Language::new();
    let ast = Ast::new();
    let current_node = plugin_engine::AstNode;
    let mut ctx = model_ctx(&lang, &ast, &current_node);

    let term = Term::Path {
        endpoints: (
            Box::new(Term::const_value(json!(1.0))),
            Box::new(Term::const_value(json!(2.0))),
        ),
        witness: Some(Box::new(Term::const_value(json!("w")))),
    };

    let result = ValueModel(TreeWalkEngine::new())
        .run(&term, json!(null), &mut ctx)
        .unwrap();

    assert_eq!(
        result,
        json!({
            "__tag__": "Path",
            "from": 1.0,
            "to": 2.0,
            "witness": "w"
        })
    );
}

#[test]
fn equiv_equal() {
    let lang = Language::new();
    let ast = Ast::new();
    let current_node = plugin_engine::AstNode;
    let mut ctx = model_ctx(&lang, &ast, &current_node);

    let term = Term::Equiv {
        a: Box::new(Term::const_value(json!(3.0))),
        b: Box::new(Term::const_value(json!(3.0))),
        path: None,
    };

    let result = ValueModel(TreeWalkEngine::new())
        .run(&term, json!(null), &mut ctx)
        .unwrap();

    assert_eq!(result, json!(true));
}

