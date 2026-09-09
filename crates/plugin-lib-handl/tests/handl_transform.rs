//! End-to-end: a `TransformDef` whose rule is `Native("handl:t.rewrite")`
//! dispatches into a `.hnl` function through `HandlNativeRewrite`
//! (ADR-0094 — handl-authored transforms).

use std::sync::Arc;

use handl_language::{Ast, AstNode, AstRef, AstShapeDef, Language, TransformDef, TransformRule};
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use handl_plugin_lib_handl::{HandlModule, HandlNativeRewrite};
use serde_json::json;

/// A trivial .hnl transform module: rewrite any matched node into kind
/// `bar`, keeping the matched node's child 0 and tagging the payload.
const T_HNL: &str = r#"
-- t: test transform module
export rewrite :: n ~> ({kind: "bar", payload: {origin: n.kind}, children: [{"__keep__": 0}]})
export decline :: n ~> null
"#;

fn test_lang() -> Language {
    let mut lang = Language::new();
    for k in ["foo", "bar", "lit", "wrap"] {
        lang.merge_ast_fragment(AstShapeDef::new(format!("ast.{k}"), k))
            .unwrap();
    }
    lang.transforms
        .merge(TransformDef::new(
            "t.mypass.foo",
            "mypass",
            "foo",
            TransformRule::Native("handl:t.rewrite".into()),
        ))
        .unwrap();
    lang.transforms
        .merge(TransformDef::new(
            "t.mypass.wrap",
            "mypass",
            "wrap",
            TransformRule::Native("handl:t.decline".into()),
        ))
        .unwrap();
    lang
}

fn test_engine() -> RewriteEngine {
    let natives = HandlNativeRewrite::new().with_module(HandlModule::new(
        "t",
        T_HNL,
        &["rewrite", "decline"],
    ));
    RewriteEngine::new().with_native_registry(Arc::new(natives))
}

#[test]
fn handl_transform_rewrites_node() {
    let lang = test_lang();
    let engine = test_engine();

    // ast: 0: lit(7), 1: foo {children: [0]}
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(7)));
    ast.push(AstNode::new("foo").with_payload(json!({"children": [0]})));
    let root = AstRef::new(1);

    let out = engine.run_pass(&lang, &ast, root, "mypass").unwrap();
    let root_node = &out.ast[out.root];
    assert_eq!(root_node.kind, "bar");
    assert_eq!(root_node.payload["origin"], json!("foo"));
    // Child 0 was kept via the __keep__ marker → remapped lit(7).
    let children = root_node.payload["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    let kept = &out.ast[AstRef::new(children[0].as_u64().unwrap() as usize)];
    assert_eq!(kept.kind, "lit");
    assert_eq!(kept.payload, json!(7));
}

#[test]
fn handl_transform_null_declines() {
    let lang = test_lang();
    let engine = test_engine();

    // ast: 0: lit(1), 1: wrap {children: [0]} — `decline` returns null so
    // the node passes through with its children remapped.
    let mut ast = Ast::new();
    ast.push(AstNode::new("lit").with_payload(json!(1)));
    ast.push(AstNode::new("wrap").with_payload(json!({"children": [0]})));
    let root = AstRef::new(1);

    let out = engine.run_pass(&lang, &ast, root, "mypass").unwrap();
    let root_node = &out.ast[out.root];
    assert_eq!(root_node.kind, "wrap");
    let children = root_node.payload["children"].as_array().unwrap();
    let kept = &out.ast[AstRef::new(children[0].as_u64().unwrap() as usize)];
    assert_eq!(kept.kind, "lit");
}

#[test]
fn handl_transform_unknown_module_errors() {
    let mut lang = Language::new();
    lang.merge_ast_fragment(AstShapeDef::new("ast.foo", "foo"))
        .unwrap();
    lang.transforms
        .merge(TransformDef::new(
            "t.mypass.foo",
            "mypass",
            "foo",
            TransformRule::Native("handl:missing.f".into()),
        ))
        .unwrap();
    let natives = HandlNativeRewrite::new().with_module(HandlModule::new(
        "t",
        T_HNL,
        &["rewrite", "decline"],
    ));
    let engine = RewriteEngine::new().with_native_registry(Arc::new(natives));

    let mut ast = Ast::new();
    ast.push(AstNode::new("foo"));
    let err = engine
        .run_pass(&lang, &ast, AstRef::new(0), "mypass")
        .unwrap_err();
    assert!(format!("{err}").contains("no HandlModule registered under 'missing'"));
}

#[test]
fn handl_transform_falls_back_for_non_handl_names() {
    use handl_plugin_engine_transform::native::NativeRewriteRegistry;

    let mut fallback = NativeRewriteRegistry::new();
    fallback.register(
        "rust.tag",
        Box::new(|ctx| {
            let idx = ctx.push_leaf("tagged", serde_json::json!({"by": "rust"}));
            Ok(handl_language::AstRef::new(idx))
        }),
    );
    let natives = HandlNativeRewrite::new()
        .with_module(HandlModule::new("t", T_HNL, &["rewrite"]))
        .with_fallback(Arc::new(fallback));

    let mut lang = Language::new();
    for k in ["foo", "tagged"] {
        lang.merge_ast_fragment(AstShapeDef::new(format!("ast.{k}"), k))
            .unwrap();
    }
    lang.transforms
        .merge(TransformDef::new(
            "t.mypass.foo",
            "mypass",
            "foo",
            TransformRule::Native("rust.tag".into()),
        ))
        .unwrap();

    let engine = RewriteEngine::new().with_native_registry(Arc::new(natives));
    let mut ast = Ast::new();
    ast.push(AstNode::new("foo"));
    let out = engine
        .run_pass(&lang, &ast, AstRef::new(0), "mypass")
        .unwrap();
    assert_eq!(out.ast[out.root].kind, "tagged");
}
