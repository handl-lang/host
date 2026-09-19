//! Scratch probe: current three-engine behavior on array/object spread.

use handl_language::{Ast, AstNode, AstShapeDef, ChildRoleSpec, EvalSchemeDef, Language};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_cranelift::CraneliftEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use plugin_engine::{ArrSource, TermData};
use serde_json::json;

fn make_ast(nodes: Vec<AstNode>) -> Ast {
    let mut ast = Ast::new();
    for n in nodes {
        ast.push(n);
    }
    ast
}

fn td(t: TermData) -> handl_language::Term {
    t.to_term().expect("TermData -> Term")
}

fn op_native(name: &str, args: Vec<TermData>) -> TermData {
    TermData::op(name, args, &ArrSource::Native, None)
}

fn child(i: u64) -> TermData {
    TermData::op("proj_child", vec![], &ArrSource::Native, Some(json!(i)))
}

fn lang() -> Language {
    let mut lang = Language::new();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.var",
            "var",
            td(TermData::op(
                "proj_env",
                vec![],
                &ArrSource::Native,
                Some(json!("$payload")),
            )),
        ))
        .unwrap();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.array",
            "array",
            td(op_native("array", vec![])),
        ))
        .unwrap();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.object",
            "object",
            td(op_native("object", vec![])),
        ))
        .unwrap();
    lang.eval
        .merge(EvalSchemeDef::new("e.rest", "rest", td(child(0))))
        .unwrap();
    // AST shape roles (ADR-0081): canonical pattern_role declarations.
    lang.ast
        .merge(AstShapeDef::new("ast.rest", "rest").with_pattern_role("rest"))
        .unwrap();
    lang.ast
        .merge(AstShapeDef::new("ast.entry", "entry").with_pattern_role("entry"))
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.object", "object").with_children(vec![ChildRoleSpec {
                role: "entries".into(),
                variadic: true,
                optional: false,
            }]),
        )
        .unwrap();
    lang
}

/// `[1, ...[10, 20], 30]` → expect `[1, 10, 20, 30]`.
fn spread_array_ast() -> Ast {
    make_ast(vec![
        AstNode::new("value").with_payload(json!(1)),  // 0
        AstNode::new("value").with_payload(json!(10)), // 1
        AstNode::new("value").with_payload(json!(20)), // 2
        AstNode::new("array").with_payload(json!({"children": [1, 2]})), // 3
        AstNode::new("rest").with_payload(json!({"children": [3]})), // 4: ...[10,20]
        AstNode::new("value").with_payload(json!(30)), // 5
        AstNode::new("array").with_payload(json!({"children": [0, 4, 5]})), // 6
    ])
}

/// `{ ...{a: 1}, c: 3 }` → expect `{a: 1, c: 3}`.
fn spread_object_ast() -> Ast {
    make_ast(vec![
        AstNode::new("var").with_payload(json!("a")), // 0
        AstNode::new("value").with_payload(json!(1)), // 1
        AstNode::new("entry").with_payload(json!({"children": [0, 1]})), // 2
        AstNode::new("object").with_payload(json!({"children": [2]})), // 3
        AstNode::new("rest").with_payload(json!({"children": [3]})), // 4: ...{a:1}
        AstNode::new("var").with_payload(json!("c")), // 5
        AstNode::new("value").with_payload(json!(3)), // 6
        AstNode::new("entry").with_payload(json!({"children": [5, 6]})), // 7
        AstNode::new("object").with_payload(json!({"children": [4, 7]})), // 8
    ])
}

#[test]
fn probe_array_spread() {
    let lang = lang();
    for (name, result) in [
        (
            "tree-walk",
            TreeWalkEngine::new().eval(&lang, &EvalRequest::new(spread_array_ast())),
        ),
        (
            "bytecode",
            BytecodeEngine::new().eval(&lang, &EvalRequest::new(spread_array_ast())),
        ),
        (
            "cranelift",
            CraneliftEngine::new().eval(&lang, &EvalRequest::new(spread_array_ast())),
        ),
    ] {
        println!("{name}: {result:?}");
    }
}

#[test]
fn probe_object_spread() {
    let lang = lang();
    for (name, result) in [
        (
            "tree-walk",
            TreeWalkEngine::new().eval(&lang, &EvalRequest::new(spread_object_ast())),
        ),
        (
            "bytecode",
            BytecodeEngine::new().eval(&lang, &EvalRequest::new(spread_object_ast())),
        ),
        (
            "cranelift",
            CraneliftEngine::new().eval(&lang, &EvalRequest::new(spread_object_ast())),
        ),
    ] {
        println!("{name}: {result:?}");
    }
}
