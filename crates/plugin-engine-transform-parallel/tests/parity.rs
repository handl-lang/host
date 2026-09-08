//! Parity, determinism, and native-wiring tests for `ParallelEngine`.

use handl_kit_data_canonical::language_from_yaml;
use handl_language::{
    Ast, AstNode, AstRef, AstShapeDef, ChildRoleSpec, Language, TransformDef,
    TransformRule,
};
use handl_plugin_engine_transform::native::{NativeRewriteRegistry, RewriteCtx};
use handl_plugin_engine_transform::request::TransformRequest;
use handl_plugin_engine_transform::TransformEngine;
use handl_plugin_engine_transform_parallel::ParallelEngine;
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use serde_json::{json, Value};
use std::sync::Arc;

fn canonical() -> Language {
    language_from_yaml().expect("canonical language should assemble")
}

fn leaf(ast: &mut Ast, kind: &str, payload: Value) -> usize {
    ast.push(AstNode::new(kind).with_payload(payload)).index()
}

fn stmt(ast: &mut Ast, kind: &str, children: &[usize]) -> usize {
    ast.push(AstNode::new(kind).with_payload(json!({ "children": children })))
        .index()
}

fn wide_block(n: usize) -> Ast {
    let mut ast = Ast::new();
    let mut stmts = Vec::new();
    for _ in 0..n {
        let cond = leaf(&mut ast, "var", json!("x"));
        let body = stmt(&mut ast, "block", &[cond]);
        stmts.push(stmt(&mut ast, "while", &[cond, body]));

        let left = leaf(&mut ast, "var", json!("a"));
        let right = leaf(&mut ast, "lit", json!(0));
        stmts.push(stmt(&mut ast, "nullish", &[left, right]));

        let obj = leaf(&mut ast, "var", json!("o"));
        let field = leaf(&mut ast, "var", json!("f"));
        stmts.push(stmt(&mut ast, "opt-chain", &[obj, field]));
    }
    let children: Vec<Value> = stmts.iter().map(|&i| Value::from(i)).collect();
    ast.push(AstNode::new("block").with_payload(json!({ "children": children })));
    ast
}

#[test]
fn parity_with_rewrite_engine_on_sugar_module() {
    let lang = canonical();
    let ast = wide_block(64);
    let root = AstRef::new(ast.len() - 1);

    let seq = RewriteEngine::new()
        .with_desugar_natives()
        .run_pass(&lang, &ast, root, "desugar")
        .expect("sequential desugar");
    let par = ParallelEngine::new()
        .with_desugar_natives()
        .with_workers(4)
        .with_threshold(1)
        .run_pass(&lang, &ast, root, "desugar")
        .expect("parallel desugar");

    assert_eq!(par.ast, seq.ast, "parallel arena differs from sequential");
    assert_eq!(par.root, seq.root);
    assert_eq!(par.trace.rewrites.len(), seq.trace.rewrites.len());
}

#[test]
fn parallel_is_deterministic() {
    let lang = canonical();
    let ast = wide_block(64);
    let root = AstRef::new(ast.len() - 1);
    let engine = ParallelEngine::new()
        .with_desugar_natives()
        .with_workers(4)
        .with_threshold(1);

    let a = engine.run_pass(&lang, &ast, root, "desugar").unwrap();
    let b = engine.run_pass(&lang, &ast, root, "desugar").unwrap();

    assert_eq!(a.ast, b.ast);
    assert_eq!(a.root, b.root);
    assert_eq!(a.trace, b.trace);
    assert_eq!(a.stats.visited, b.stats.visited);
    assert_eq!(a.stats.rewritten, b.stats.rewritten);
    assert_eq!(a.stats.per_rule, b.stats.per_rule);
}

#[test]
fn native_registry_is_wired() {
    let mut lang = Language::new();
    for k in ["a", "b", "block"] {
        lang.merge_ast_fragment(AstShapeDef::new(format!("ast.{k}"), k))
            .unwrap();
    }
    // Give block a variadic statements role so children are recognized as refs.
    lang.merge_ast_fragment(
        AstShapeDef::new("ast.block", "block").with_children(vec![ChildRoleSpec {
            role: "statements".into(),
            variadic: true,
        }]),
    )
    .unwrap();
    lang.transforms
        .merge(TransformDef::new(
            "t.a",
            "p",
            "a",
            TransformRule::Native("mk.b".into()),
        ))
        .unwrap();

    let mut reg = NativeRewriteRegistry::new();
    reg.register(
        "mk.b",
        Box::new(|ctx: &mut RewriteCtx| {
            Ok(AstRef::new(ctx.push_leaf("b", json!("made"))))
        }),
    );

    let mut ast = Ast::new();
    let a0 = ast.push(AstNode::new("a")).index();
    let a1 = ast.push(AstNode::new("a")).index();
    let a2 = ast.push(AstNode::new("a")).index();
    ast.push(AstNode::new("block").with_payload(json!({ "children": [a0, a1, a2] })));
    let root = AstRef::new(ast.len() - 1);

    let engine = ParallelEngine::new()
        .with_workers(4)
        .with_threshold(1)
        .with_natives(Arc::new(reg));
    let req = TransformRequest::new(ast, root, "p");
    let out = engine.transform(&lang, &req).expect("parallel pass");

    let top = out.ast.at(out.root).unwrap();
    assert_eq!(top.kind, "block");
    let children = top.payload["children"].as_array().unwrap();
    for c in children {
        let idx = c.as_u64().unwrap() as usize;
        assert_eq!(out.ast.at(AstRef::new(idx)).unwrap().kind, "b");
    }
    assert_eq!(out.stats.rewritten, 3);
}
