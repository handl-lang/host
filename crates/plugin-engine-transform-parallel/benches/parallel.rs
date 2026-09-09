//! Criterion bench for `ParallelEngine` vs `RewriteEngine` on mixed modules.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use handl_kit_data_canonical::language_from_yaml;
use handl_language::{Ast, AstNode, AstRef, Language};
use handl_plugin_engine_transform_parallel::ParallelEngine;
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use serde_json::{Value, json};

fn canonical_lang() -> Language {
    language_from_yaml().expect("canonical language should assemble")
}

fn leaf(ast: &mut Ast, kind: &str, payload: Value) -> usize {
    ast.push(AstNode::new(kind).with_payload(payload)).index()
}

fn stmt(ast: &mut Ast, kind: &str, children: &[usize]) -> usize {
    ast.push(AstNode::new(kind).with_payload(json!({ "children": children })))
        .index()
}

/// A wide module of `n` top-level statements with a realistic mix of forms
/// that exercise the native desugar registry (`while`, `??`, `?.`) alongside
/// plain assignments.
fn mixed_module(n: usize) -> Ast {
    let mut ast = Ast::new();
    let mut stmts = Vec::with_capacity(n);
    for i in 0..n {
        match i % 4 {
            0 => {
                let cond = leaf(&mut ast, "var", json!("x"));
                let body = stmt(&mut ast, "block", &[cond]);
                stmts.push(stmt(&mut ast, "while", &[cond, body]));
            }
            1 => {
                let left = leaf(&mut ast, "var", json!("a"));
                let right = leaf(&mut ast, "lit", json!(0));
                stmts.push(stmt(&mut ast, "nullish", &[left, right]));
            }
            2 => {
                let obj = leaf(&mut ast, "var", json!("o"));
                let field = leaf(&mut ast, "var", json!("f"));
                stmts.push(stmt(&mut ast, "opt-chain", &[obj, field]));
            }
            _ => {
                let target = leaf(&mut ast, "var", json!("t"));
                let value = leaf(&mut ast, "lit", json!(i));
                stmts.push(stmt(&mut ast, "assign", &[target, value]));
            }
        }
    }
    let children: Vec<Value> = stmts.iter().map(|&i| Value::from(i)).collect();
    ast.push(AstNode::new("block").with_payload(json!({ "children": children })));
    ast
}

fn bench_sweep(c: &mut Criterion) {
    let lang = canonical_lang();
    let seq = RewriteEngine::new().with_desugar_natives();
    let par = ParallelEngine::new().with_desugar_natives();

    let mut group = c.benchmark_group("desugar_sweep");
    for &n in [500, 2_000, 5_000, 10_000, 50_000].iter() {
        let ast = mixed_module(n);
        let root = AstRef::new(ast.len() - 1);

        group.bench_with_input(BenchmarkId::new("sequential", n), &ast, |b, ast| {
            b.iter(|| {
                seq.run_pass(&lang, black_box(ast), root, "desugar")
                    .unwrap()
            });
        });

        group.bench_with_input(BenchmarkId::new("parallel", n), &ast, |b, ast| {
            b.iter(|| {
                par.run_pass(&lang, black_box(ast), root, "desugar")
                    .unwrap()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_sweep);
criterion_main!(benches);
