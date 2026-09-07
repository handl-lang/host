//! Criterion bench for `ParallelEngine` vs `RewriteEngine` on a wide module.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use handl_kit_data_canonical::language_from_yaml;
use handl_language::{Ast, AstNode, AstRef, Language};
use handl_plugin_engine_transform_parallel::ParallelEngine;
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use serde_json::json;

const STATEMENTS: usize = 500;

fn canonical_lang() -> Language {
    language_from_yaml().expect("canonical language should assemble")
}

/// A wide module of 500 top-level `while`/`??` statements.
fn wide_module() -> Ast {
    let mut ast = Ast::new();
    let mut stmt_roots = Vec::new();
    for _ in 0..STATEMENTS {
        let cond = ast.push(AstNode::new("var").with_payload(json!("x"))).index();
        let body = ast.push(AstNode::new("block").with_payload(json!({"children": [cond]}))).index();
        stmt_roots.push(
            ast.push(AstNode::new("while").with_payload(json!({"children": [cond, body]})))
                .index(),
        );
        let left = ast.push(AstNode::new("var").with_payload(json!("a"))).index();
        let right = ast.push(AstNode::new("lit").with_payload(json!(0))).index();
        stmt_roots.push(
            ast.push(AstNode::new("nullish").with_payload(json!({"children": [left, right]})))
                .index(),
        );
    }
    // Module root wraps all statements.
    ast.push(AstNode::new("module").with_payload(json!({"children": stmt_roots})));
    ast
}

fn bench_parallel_vs_sequential(c: &mut Criterion) {
    let lang = canonical_lang();
    let ast = wide_module();
    let root = AstRef::new(ast.len() - 1);

    let seq = RewriteEngine::new().with_desugar_natives();
    let par = ParallelEngine::new().with_desugar_natives();

    c.bench_function("rewrite_sequential_500", |b| {
        b.iter(|| {
            seq.run_pass(&lang, black_box(&ast), root, "desugar").unwrap();
        })
    });

    c.bench_function("rewrite_parallel_500", |b| {
        b.iter(|| {
            par.run_pass(&lang, black_box(&ast), root, "desugar").unwrap();
        })
    });
}

criterion_group!(benches, bench_parallel_vs_sequential);
criterion_main!(benches);
