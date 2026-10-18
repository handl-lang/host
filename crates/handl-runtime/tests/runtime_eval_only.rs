//! Integration test: eval-only runtime — a pre-parsed `Ast` is fed to `eval`
//! without any lex or parse engines bound (ADR-0044 §2: "A `Runtime` may offer
//! `eval` alone ... without pulling in a lexer or parser").
//!
//! This is the use case for a JIT/cranelift runtime that compiles a pre-parsed
//! `Ast`: the runtime has no lex or parse engine, `lex` returns
//! `LangError::NotImplemented`, and `eval` works on the provided `Ast`.

use handl_language::{Ast, AstNode, EvalSchemeDef, LangError, Language, Source, Tokens, Value};
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};
use hnl_engine::{ArrSource, TermData};
use serde_json::json;
use std::rc::Rc;

/// Build a registry with the tree-walk eval engine registered under
/// `"tree-walk"`.
fn test_registry() -> Rc<EngineRegistry> {
    let mut reg = EngineRegistry::new();
    reg.register_eval(
        "tree-walk",
        Box::new(hnl_engine_exec_tree_walk::TreeWalkEngine::new()),
    );
    Rc::new(reg)
}

/// Build a language with eval schemes for `lit` (no scheme → payload),
/// `binary.add` (arr add), and a `let`/`var` pair for scope testing.
fn eval_lang() -> Language {
    let mut lang = Language::new();
    // binary.add → arr("add", [proj_child 0, proj_child 1])
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.add",
            "binary.add",
            TermData::op(
                "add",
                vec![
                    TermData::op(
                        "proj_child",
                        vec![],
                        &ArrSource::Native,
                        Some(Value::from(0u64)),
                    ),
                    TermData::op(
                        "proj_child",
                        vec![],
                        &ArrSource::Native,
                        Some(Value::from(1u64)),
                    ),
                ],
                &ArrSource::Native,
                None,
            )
            .to_term()
            .expect("TermData -> Term"),
        ))
        .unwrap();
    // var → arr("proj_env", lit: "$payload") reads name from payload
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.var",
            "var",
            TermData::op(
                "proj_env",
                vec![],
                &ArrSource::Native,
                Some(Value::String("$payload".into())),
            )
            .to_term()
            .expect("TermData -> Term"),
        ))
        .unwrap();
    // let → value >>> arr("extend_env", lit: "$payload") >>> body
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.let",
            "let",
            TermData::seq(
                TermData::seq(
                    TermData::op(
                        "proj_child",
                        vec![],
                        &ArrSource::Native,
                        Some(Value::from(0u64)),
                    ),
                    TermData::op(
                        "extend_env",
                        vec![],
                        &ArrSource::Native,
                        Some(Value::String("$payload".into())),
                    ),
                ),
                TermData::op(
                    "proj_child",
                    vec![],
                    &ArrSource::Native,
                    Some(Value::from(1u64)),
                ),
            )
            .to_term()
            .expect("TermData -> Term"),
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
        hnl_engine_exec_tree_walk::TreeWalkEngine::new(),
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
