//! Pipeline benchmarks.
//!
//! Measures the end-to-end path through the assembled kit — parse, then eval —
//! so the two heavy stages can be read off separately. This complements
//! `kit-canonical/benches/perf.rs`, which measures a long `|>` chain; here the
//! input is the richer mixed-literal `SAMPLE`, so lexing/parsing cost
//! dominates rather than composition depth.
//!
//! The pre-refactor `pipeline` bench drove `handl-compiler`'s
//! `DriverPipeline`/`Session`, which no longer exists; `Kit` is the supported
//! end-to-end seam now.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use handl_bench::{EVAL_SAMPLE, SAMPLE};
use handl_kit_canonical::Kit;

fn bench_parse(c: &mut Criterion) {
    let kit = Kit::new().expect("kit assembly");
    c.bench_function("pipeline/parse", |b| {
        b.iter(|| kit.parse(black_box(SAMPLE)).expect("sample parses"));
    });
}

fn bench_parse_and_eval(c: &mut Criterion) {
    let kit = Kit::new().expect("kit assembly");
    // `EVAL_SAMPLE`, not `SAMPLE`: eval needs every name bound.
    c.bench_function("pipeline/parse_and_eval", |b| {
        b.iter(|| kit.eval(black_box(EVAL_SAMPLE)).expect("sample evaluates"));
    });
}

criterion_group!(pipeline_benches, bench_parse, bench_parse_and_eval);
criterion_main!(pipeline_benches);
