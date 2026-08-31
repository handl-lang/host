//! Integration test: eval-only runtime — a pre-parsed `Ast` is fed to `eval`
//! without any lex or parse engines bound (ADR-0044 §2: "A `Runtime` may offer
//! `eval` alone ... without pulling in a lexer or parser").
//!
//! This is the use case for a JIT/cranelift runtime that compiles a pre-parsed
//! `Ast`: the runtime has no lex or parse engine, `lex` returns
//! `LangError::NotImplemented`, and `eval` works on the provided `Ast`.

use handl_language::{
    Ast, AstNode, EvalSchemeDef, LangError, Language, Scheme, Source, Tokens, Value,
};
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};
use serde_json::json;
use std::rc::Rc;

/// Build a registry with the tree-walk eval engine registered under
/// `"tree-walk"`.
fn test_registry() -> Rc<EngineRegistry> {
    let mut reg = EngineRegistry::new();
    reg.register_eval(
        "tree-walk",
        Box::new(handl_plugin_engine_eval_tree_walk::TreeWalkEngine::new()),
    );
    Rc::new(reg)
}

/// Build a language with eval schemes for `lit` (no scheme → payload),
/// `binary.add` (Prim add), and a `let`/`var` pair for scope testing.
fn eval_lang() -> Language {
    let mut lang = Language::new();
    // binary.add → add children 0 and 1
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.add",
            "binary.add",
            Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
        ))
        .unwrap();
    // var → read name from payload ($payload convention)
    lang.eval
        .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
        .unwrap();
    // let → bind name from payload, value = child 0, body = child 1
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.let",
            "let",
            Scheme::Bind {
                name: "$payload".into(),
                value: Box::new(Scheme::child(0)),
                body: Box::new(Scheme::child(1)),
            },
        ))
        .unwrap();
    lang
}

#[test]
fn eval_only_runtime_evaluates_pre_parsed_ast() {
    let lang = eval_lang();
    // Only an eval engine is bound — no lex or parse engine.
    let config = RuntimeConfig::new().with_eval("tree-walk");
    let rt =
        Runtime::from_registry(lang, &test_registry(), &config).expect("config should resolve");

    // Ast: 0: lit(2), 1: lit(3), 2: binary.add {children: [0, 1]}
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(2)));
    ast.push(AstNode::new("lit").with_payload(json!(3)));
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})));

    let result = rt.eval(&ast).expect("eval should succeed");
    assert_eq!(result, json!(5));
}

#[test]
fn eval_only_runtime_lex_returns_not_implemented() {
    let lang = eval_lang();
    let config = RuntimeConfig::new().with_eval("tree-walk");
    let rt =
        Runtime::from_registry(lang, &test_registry(), &config).expect("config should resolve");

    let err = rt.lex(&Source::new("1 + 2")).unwrap_err();
    assert_eq!(err, LangError::NotImplemented);
}

#[test]
fn eval_only_runtime_parse_returns_not_implemented() {
    let lang = eval_lang();
    let config = RuntimeConfig::new().with_eval("tree-walk");
    let rt =
        Runtime::from_registry(lang, &test_registry(), &config).expect("config should resolve");

    let err = rt.parse(&Tokens::new()).unwrap_err();
    assert_eq!(err, LangError::NotImplemented);
}

#[test]
fn eval_only_runtime_run_returns_not_implemented_from_lex() {
    let lang = eval_lang();
    let config = RuntimeConfig::new().with_eval("tree-walk");
    let rt =
        Runtime::from_registry(lang, &test_registry(), &config).expect("config should resolve");

    // run() tries lex first, which is NotImplemented.
    let err = rt.run(&Source::new("1 + 2")).unwrap_err();
    assert_eq!(err, LangError::NotImplemented);
}

#[test]
fn eval_only_runtime_with_let_binding() {
    let lang = eval_lang();
    let config = RuntimeConfig::new().with_eval("tree-walk");
    let rt =
        Runtime::from_registry(lang, &test_registry(), &config).expect("config should resolve");

    // Ast: 0: lit(42), 1: var("x"), 2: let {name: "x", children: [0, 1]}
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(42)));
    ast.push(AstNode::new("var").with_payload(json!("x")));
    ast.push(AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})));

    let result = rt.eval(&ast).expect("eval should succeed");
    assert_eq!(result, json!(42));
}

#[test]
fn eval_only_runtime_with_manual_engine_binding() {
    // Verify that manually binding an eval engine via the builder (not
    // from_config) also works for an eval-only runtime.
    let lang = eval_lang();
    let rt = Runtime::new(lang).with_eval_engine(Box::new(
        handl_plugin_engine_eval_tree_walk::TreeWalkEngine::new(),
    ));

    // lex/parse are not bound.
    assert_eq!(
        rt.lex(&Source::new("x")).unwrap_err(),
        LangError::NotImplemented
    );
    assert_eq!(
        rt.parse(&Tokens::new()).unwrap_err(),
        LangError::NotImplemented
    );

    // eval works.
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(99)));
    let result = rt.eval(&ast).unwrap();
    assert_eq!(result, Value::from(99));
}
