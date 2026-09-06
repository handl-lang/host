//! Integration test: proof erasure in `Runtime::run_checked`.
//!
//! Stubs out the lex and parse stages and installs a typecheck engine that
//! reports the root node as a `type.refl` proof. `run_checked` should erase
//! that node before eval, so the final value is `null`.

use handl_language::{Ast, AstNode, LangError, Language, Source, Tokens, Value};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex::{LexerEngine, LexerRequest};
use handl_plugin_engine_parse::{ParserContext, ParserEngine, ParserRequest};
use handl_runtime::{Runtime, TypecheckResult};
use serde_json::json;
use std::collections::HashMap;

#[derive(Clone)]
struct StubLexer;

impl LexerEngine for StubLexer {
    fn lex(&self, _lang: &Language, _req: &LexerRequest) -> Result<Tokens, LangError> {
        Ok(Tokens::new())
    }

    fn box_clone(&self) -> Box<dyn LexerEngine> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
struct StubParser {
    ast: Ast,
}

impl ParserEngine for StubParser {
    fn expression(
        &self,
        _ctx: &mut ParserContext<'_, '_, Self>,
        _rbp: u8,
    ) -> Result<handl_language::AstRef, LangError> {
        Ok(handl_language::AstRef::new(0))
    }

    fn parse(&self, _lang: &Language, _req: &ParserRequest) -> Result<Ast, LangError> {
        Ok(self.ast.clone())
    }
}

#[derive(Clone)]
struct StubTypecheck {
    node_types: HashMap<usize, Value>,
}

impl handl_plugin_engine_typecheck::TypecheckEngine for StubTypecheck {
    fn typecheck(
        &self,
        _lang: &Language,
        _req: &handl_plugin_engine_typecheck::TypecheckRequest,
    ) -> Result<TypecheckResult, LangError> {
        Ok(TypecheckResult {
            typ: json!({"__tag__": "type.refl", "__registry__": "handl.canonical"}),
            diagnostics: vec![],
            node_types: self.node_types.clone(),
        })
    }

    fn box_clone(&self) -> Box<dyn handl_plugin_engine_typecheck::TypecheckEngine> {
        Box::new(self.clone())
    }
}

fn proof_value(tag: &str) -> Value {
    json!({"__tag__": tag, "__registry__": "handl.canonical"})
}

#[test]
fn run_checked_erases_proof_typed_root() {
    // The program is a single `value` node carrying 42. The stub typechecker
    // reports its type as `type.refl`, so the erase pass should rewrite it to
    // `erased` before evaluation.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(Value::from(42)));

    let mut node_types = HashMap::new();
    node_types.insert(0, proof_value("type.refl"));

    let rt = Runtime::new(Language::new())
        .with_lex_engine(Box::new(StubLexer))
        .with_parse_engine(StubParser { ast })
        .with_typecheck_engine(Box::new(StubTypecheck { node_types }))
        .with_eval_engine(Box::new(TreeWalkEngine::new()));

    let result = rt.run_checked(&Source::new("42")).expect("run_checked should succeed");
    assert!(result.is_null(), "proof-typed root should be erased to null, got {:?}", result);
}

#[test]
fn run_checked_preserves_computational_nodes() {
    // A `value` node whose type is `type.number` should not be erased.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(Value::from(42)));

    let mut node_types = HashMap::new();
    node_types.insert(
        0,
        json!({"__tag__": "type.number", "__registry__": "handl.canonical"}),
    );

    let rt = Runtime::new(Language::new())
        .with_lex_engine(Box::new(StubLexer))
        .with_parse_engine(StubParser { ast })
        .with_typecheck_engine(Box::new(StubTypecheck { node_types }))
        .with_eval_engine(Box::new(TreeWalkEngine::new()));

    let result = rt.run_checked(&Source::new("42")).expect("run_checked should succeed");
    assert_eq!(result, Value::from(42));
}
