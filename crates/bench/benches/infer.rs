//! Inference benchmarks.
//!
//! The pre-refactor `infer` bench targeted
//! `handl-typecheck-infer::{infer, infer_cached, parse_expr, FunctionTable,
//! ParseCache}` — a crate that is gone. Inference now lives in the `.hnl`
//! model behind `Kit::typecheck_handl_with_prelude` (ADR-0105), which is a
//! *different* entry point from the `typecheck` bench's
//! `hnl-engine-typecheck::check_module` (the bootstrap checker), so the two
//! benches do not overlap.
//!
//! `infer_cached` measured reuse of previously-computed inference state. The
//! modern analogue is the `Kit`'s lazily-built Handl typecheck engine, which
//! is constructed on first use and reused afterwards — so the bench warms the
//! kit before measuring; the measured cost is inference with the engine
//! already built.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use handl_bench::INFERENCE_SAMPLE;
use handl_kit_canonical::Kit;
use std::collections::HashSet;

fn bench_infer(c: &mut Criterion) {
    let kit = Kit::new().expect("canonical kit assembles");
    // No extra prelude: `INFERENCE_SAMPLE` binds every name it uses.
    let prelude = HashSet::new();

    // Warm the kit's lazily-built Handl engine so the measurement is
    // inference, not first-call model construction (the warm/cold distinction
    // the old `infer_cached` drew).
    kit.typecheck_handl_with_prelude(INFERENCE_SAMPLE, &prelude)
        .expect("inference sample typechecks");

    c.bench_function("infer/check_handl", |b| {
        b.iter(|| {
            kit.typecheck_handl_with_prelude(black_box(INFERENCE_SAMPLE), &prelude)
                .expect("inference sample typechecks")
        });
    });
}

criterion_group!(infer_benches, bench_infer);
criterion_main!(infer_benches);
