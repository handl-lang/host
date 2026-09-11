//! Law/rewrite lookup tests for `ValueModel::rewrite` (ADR-0100).
//!
//! Verifies that a `Named` rewrite witness is looked up in the
//! language's law/rewrite registry and that the stored source and
//! target terms are run and compared.

use handl_language::{ArrSource, Ast, Language, Term, Value};
use handl_plugin_engine_exec_tree_walk::{TreeWalkEngine, ValueModel};
use plugin_engine::{Model, ModelCtx, RewriteResult, RewriteWitness, TermData};
use serde_json::json;

fn t(td: TermData) -> Term {
    td.to_term().expect("TermData -> Term conversion")
}

fn model_ctx<'a>(
    lang: &'a Language,
    ast: &'a Ast,
    node: &'a plugin_engine::AstNode,
) -> ModelCtx<'a> {
    let lang_view: &dyn plugin_engine::LanguageView = lang;
    let ast_view: &dyn plugin_engine::AstView = ast;
    ModelCtx {
        ast: ast_view,
        lang: lang_view,
        current_node: node,
        node_ref: 0,
        handler_stack: vec![],
    }
}

fn add_zero_law(x: f64) -> Value {
    let source = t(TermData::op(
        "add",
        vec![TermData::arr(json!(0.0)), TermData::arr(json!(x))],
        &ArrSource::Native,
        None,
    ));
    let target = t(TermData::arr(json!(x)));
    json!({
        "source": serde_json::to_value(source).unwrap(),
        "target": serde_json::to_value(target).unwrap(),
    })
}

fn one_to_two_rewrite() -> Value {
    let source = t(TermData::arr(json!(1.0)));
    let target = t(TermData::arr(json!(2.0)));
    json!({
        "source": serde_json::to_value(source).unwrap(),
        "target": serde_json::to_value(target).unwrap(),
    })
}

#[test]
fn named_law_lookup_returns_equal() {
    let x = 7.0;
    let mut lang = Language::new();
    lang.register_law("add_zero", add_zero_law(x));

    let ast = Ast::new();
    let current_node = plugin_engine::AstNode;
    let mut ctx = model_ctx(&lang, &ast, &current_node);

    let incoming = json!(null);
    let source = t(TermData::arr(json!(null)));
    let target = t(TermData::arr(json!(null)));
    let result = ValueModel::new(TreeWalkEngine::new()).rewrite(
        &source,
        &target,
        &RewriteWitness::Named("add_zero".to_string()),
        &incoming,
        &mut ctx,
    );

    assert!(matches!(result, Ok(RewriteResult::Equal(v)) if v == json!(x)));
}

#[test]
fn named_rewrite_lookup_returns_rewritten() {
    let mut lang = Language::new();
    lang.register_rewrite("one_to_two", one_to_two_rewrite());

    let ast = Ast::new();
    let current_node = plugin_engine::AstNode;
    let mut ctx = model_ctx(&lang, &ast, &current_node);

    let incoming = json!(null);
    let source = t(TermData::arr(json!(null)));
    let target = t(TermData::arr(json!(null)));
    let result = ValueModel::new(TreeWalkEngine::new()).rewrite(
        &source,
        &target,
        &RewriteWitness::Named("one_to_two".to_string()),
        &incoming,
        &mut ctx,
    );

    assert!(matches!(result, Ok(RewriteResult::Rewritten(v)) if v == json!(2.0)));
}

#[test]
fn unknown_named_law_falls_back_to_equality_check() {
    let lang = Language::new();
    let ast = Ast::new();
    let current_node = plugin_engine::AstNode;
    let mut ctx = model_ctx(&lang, &ast, &current_node);

    let incoming = json!(null);
    let source = t(TermData::arr(json!(1.0)));
    let target = t(TermData::arr(json!(1.0)));
    let result = ValueModel::new(TreeWalkEngine::new()).rewrite(
        &source,
        &target,
        &RewriteWitness::Named("missing".to_string()),
        &incoming,
        &mut ctx,
    );

    assert!(matches!(result, Ok(RewriteResult::Equal(v)) if v == json!(1.0)));
}
