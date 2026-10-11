//! Scaling of the canonical kit on a long module: how parse and eval cost grow
//! with the number of statements.
//!
//! Note: this bench formerly built a long `|>` pipeline
//! (`"1 |> f |> f |> …"`). `|>` was **removed** from the canonical grammar
//! (`handl-kit-data-canonical/data/ast.yaml`, "Removed: … pipe …"), so
//! `bench_parse` panicked inside `unwrap()` on its first iteration and
//! `bench_eval` silently benched `unwrap_or_default()` — the *error* path, not
//! evaluation. Both now build a flat chain of applications, which parses and
//! evaluates.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use handl_kit_canonical::Kit;

/// `f = x ~> x; a0 = f 0; a1 = f 1; …; a{n-1} = f {n-1};` — `n` applications
/// with flat nesting, so scale comes from module length rather than parser
/// recursion depth.
fn chain_module(n: usize) -> String {
    let mut s = String::from("f = x ~> x;");
    for i in 0..n {
        s.push_str(&format!(" a{i} = f {i};"));
    }
    s
}

fn bench_parse(c: &mut Criterion) {
    let kit = Kit::new().expect("kit assembly should succeed");
    let mut group = c.benchmark_group("parse_long_chain");
    for n in [100, 1000] {
        let input = chain_module(n);
        group.bench_with_input(format!("{n}"), &input, |b, input| {
            b.iter(|| kit.parse(black_box(input)).expect("chain parses"));
        });
    }
    group.finish();
}

fn bench_eval(c: &mut Criterion) {
    let kit = Kit::new().expect("kit assembly should succeed");
    let mut group = c.benchmark_group("eval_long_chain");
    for n in [100, 1000] {
        let input = chain_module(n);
        group.bench_with_input(format!("{n}"), &input, |b, input| {
            b.iter(|| kit.eval(black_box(input)).expect("chain evaluates"));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse, bench_eval);
criterion_main!(benches);
