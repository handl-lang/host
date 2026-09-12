//! Path, equivalence, and transport semantics for `ValueModel` (ADR-0100).

use handl_language::{Ast, Language, Term};
use handl_plugin_engine_exec_tree_walk::{TreeWalkEngine, ValueModel};
use plugin_engine::{Model, ModelCtx, TermData};
use serde_json::json;

/// Convert `TermData` back to `Term` for engine APIs that require `Term`.
fn td(t: TermData) -> Term {
    t.to_term().expect("TermData -> Term conversion")
}

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

    let term = td(TermData::path(
        TermData::arr(json!(1.0)),
        TermData::arr(json!(2.0)),
        Some(TermData::arr(json!("w"))),
    ));

    let result = ValueModel::new(TreeWalkEngine::new())
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

    let term = td(TermData::equiv(
        TermData::arr(json!(3.0)),
        TermData::arr(json!(3.0)),
        None,
    ));

    let result = ValueModel::new(TreeWalkEngine::new())
        .run(&term, json!(null), &mut ctx)
        .unwrap();

    assert_eq!(result, json!(true));
}
