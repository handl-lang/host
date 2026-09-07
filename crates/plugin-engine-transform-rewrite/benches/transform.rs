//! Criterion bench for `RewriteEngine::run_pass("desugar")` (ADR-0094
//! "Performance"): the "performant" claim is measured, not asserted.
//!
//! A large synthetic arena (~10k nodes) is built programmatically —
//! children always precede parents, matching the Pratt arena layout the
//! bottom-up walk relies on. Two shapes are measured:
//!
//! - `desugar_walk_10k`: pure pass-through walk over kinds with no rules
//!   (measures the O(nodes) `remap` + single `HashMap` lookup per node).
//! - `desugar_sugar_10k`: a mix of `nullish`/`opt-chain`/`while` sugar
//!   nodes over `value` leaves (exercises the `desugar.*` native registry).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use handl_language::{Ast, AstNode, Language};
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use serde_json::json;

const NODES: usize = 10_000;

fn canonical_lang() -> Language {
    handl_kit_data_canonical::language_from_yaml().expect("canonical language should assemble")
}

/// Push `value` leaves and combine them under `kind`, until the arena holds
/// at least `target` nodes. Children always precede parents.
fn grow(ast: &mut Ast, kind: &str, target: usize) {
    while ast.len() + 3 <= target {
        let l = ast
            .push(AstNode::new("value").with_payload(json!(1)))
            .index();
        let r = ast
            .push(AstNode::new("value").with_payload(json!(2)))
            .index();
        ast.push(AstNode::new(kind).with_payload(json!({ "children": [l, r] })));
    }
    while ast.len() < target {
        ast.push(AstNode::new("value").with_payload(json!(0)));
    }
}

/// ~10k nodes of kinds no desugar rule or native handles.
fn walk_ast(target: usize) -> Ast {
    let mut ast = Ast::new();
    grow(&mut ast, "binary.add", target);
    ast
}

/// ~10k nodes mixing pass-through `binary.add` with sugar kinds handled by
/// the `desugar.*` natives (`nullish`, `opt-chain`, `while`).
fn sugar_ast(target: usize) -> Ast {
    let mut ast = Ast::new();
    let kinds = ["binary.add", "nullish", "binary.add", "opt-chain", "while"];
    let mut i = 0;
    while ast.len() + 3 <= target {
        let l = ast
            .push(AstNode::new("value").with_payload(json!(i)))
            .index();
        let r = ast
            .push(AstNode::new("value").with_payload(json!(i + 1)))
            .index();
        ast.push(AstNode::new(kinds[i % kinds.len()]).with_payload(json!({ "children": [l, r] })));
        i += 1;
    }
    while ast.len() < target {
        ast.push(AstNode::new("value").with_payload(json!(0)));
    }
    ast
}

fn bench_desugar_walk(c: &mut Criterion) {
    let lang = canonical_lang();
    let engine = RewriteEngine::new().with_desugar_natives();
    let mut group = c.benchmark_group("desugar_pass");
    group.bench_function("walk_10k_nodes", |b| {
        b.iter_with_setup(
            || walk_ast(NODES),
            |ast| {
                let root = handl_language::AstRef::new(ast.len() - 1);
                engine
                    .run_pass(&lang, black_box(&ast), root, "desugar")
                    .unwrap()
            },
        );
    });
    group.bench_function("sugar_10k_nodes", |b| {
        b.iter_with_setup(
            || sugar_ast(NODES),
            |ast| {
                let root = handl_language::AstRef::new(ast.len() - 1);
                engine
                    .run_pass(&lang, black_box(&ast), root, "desugar")
                    .unwrap()
            },
        );
    });
    group.finish();
}

criterion_group!(benches, bench_desugar_walk);
criterion_main!(benches);
