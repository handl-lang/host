//! Typechecker benchmarks.
//!
//! Measures `hnl-engine-typecheck::check_module` directly, so a regression
//! is attributable to the checker rather than to parse or eval. This is the
//! `typecheck` bench the pre-refactor suite had; it targeted
//! `handl-typecheck-{core,infer,schema-json}`, all of which are gone, so it is
//! a re-target onto the current entry point rather than a revert.
//!
//! `check_module` takes the `Language` from its caller (the cyclic-dependency
//! workaround documented in `AGENTS.md`) — the bench supplies the canonical
//! language, which is exactly what a host does.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use handl_bench::SAMPLE;

/// The canonical language, assembled from the embedded YAML fragments.
fn canonical_lang() -> handl_language::Language {
    handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles")
}

fn bench_typecheck(c: &mut Criterion) {
    let lang = canonical_lang();
    c.bench_function("typecheck/check_module", |b| {
        b.iter(|| {
            hnl_engine_typecheck::check_module(&lang, black_box(SAMPLE))
                .expect("sample typechecks")
        });
    });
}

criterion_group!(typecheck_benches, bench_typecheck);
criterion_main!(typecheck_benches);
