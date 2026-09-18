//! Tests for the `handle <expr> with <handler>` surface syntax.

use handl_kit_data_canonical;
use handl_language::{Ast, AstNode, Language, Source};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, Runtime};
use std::rc::Rc;

fn full_language() -> Language {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    kit.lang().clone()
}

fn full_runtime() -> Runtime {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);
    let mut config = kit.runtime_config().clone();
    config.typecheck.clear();
    Runtime::from_registry(kit.lang().clone(), &registry, &config).expect("runtime binds engines")
}

fn parse(src: &str) -> Ast {
    let rt = full_runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex should succeed");
    rt.parse(&toks).expect("parse should succeed")
}

fn root(ast: &Ast) -> &AstNode {
    ast.nodes().last().expect("ast is nonempty")
}

fn children<'a>(ast: &'a Ast, node: &'a AstNode) -> Vec<&'a AstNode> {
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("node has children")
        .iter()
        .map(|i| &ast.nodes()[i.as_u64().unwrap() as usize])
        .collect()
}

#[test]
fn handle_tokens_exist() {
    let lang = full_language();
    for id in ["token.keyword.handle", "token.keyword.with"] {
        assert!(lang.tokens.get_by_id(id).is_some(), "missing {id}");
    }
}

#[test]
fn handle_grammar_row_exists() {
    let lang = full_language();
    assert!(
        lang.grammar.get_by_id("grammar.handle.prefix").is_some(),
        "missing grammar.handle.prefix"
    );
}

#[test]
fn handle_ast_shape_exists() {
    let lang = full_language();
    let shape = lang
        .ast
        .get_by_id("ast.handle")
        .unwrap_or_else(|| panic!("missing ast.handle"));
    let declared: Vec<&str> = shape
        .children
        .as_ref()
        .expect("shape declares child roles")
        .iter()
        .map(|c| c.role.as_str())
        .collect();
    assert_eq!(declared, vec!["target", "impl"]);
}

#[test]
fn handle_expression_parses() {
    let ast = parse("handle add(1, 2) with NumberHandler");
    let r = root(&ast);
    assert_eq!(r.kind, "handle");
    let cs = children(&ast, r);
    assert_eq!(cs.len(), 2);
    assert_eq!(cs[0].kind, "call");
    assert_eq!(cs[1].kind, "var");
    assert_eq!(cs[1].payload, serde_json::json!("NumberHandler"));
}
