//! Object/sym-graph "core" benchmarks.
//!
//! The pre-refactor suite had a `core` bench targeting
//! `handl-typecheck-core::{SchemaInterpreter, Type, is_compatible}` plus
//! `schema_json`. Those crates are gone. Post-ADR-0106 the type universe is
//! the **object/sym graph** in `handl-value`, and the questions the old bench
//! asked — "is this schema value well-formed / is this type compatible" —
//! are answered by *portability* (`check_portable`) and *nominal ancestry*
//! (`has_ancestor` / `is_descendant_of`).
//!
//! So this is a re-target, not a revert. The micro-operations measured are
//! the graph's hot paths:
//!
//! - `decode`  — `serde_json::Value` → `TaggedValue` (wire → graph)
//! - `encode`  — `tagged_to_value` (graph → wire)
//! - `portable` — `check_portable` (the `SchemaInterpreter` successor)
//! - `ancestry` — `has_ancestor` (the `is_compatible` successor)

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use handl_value::{
    TaggedValue, check_portable, has_ancestor, sym, sym_from_parent_value, tagged_to_value,
};
use serde_json::json;

/// A document exercising the graph: an array, a nested record, and a nominal
/// symbol (`__sym__`).
fn document() -> serde_json::Value {
    json!({
        "a": [1, 2, 3],
        "b": { "c": "d", "e": [true, null] },
        "sym": { "__sym__": { "handle": 1, "name": "base", "parent": null } },
    })
}

fn bench_core(c: &mut Criterion) {
    let doc = document();
    let tagged = TaggedValue::from(doc.clone());
    let base = sym("base");
    let child = sym_from_parent_value("child", &base).expect("child sym is well-formed");

    c.bench_function("core/decode_value_to_tagged", |b| {
        b.iter(|| TaggedValue::from(black_box(doc.clone())));
    });
    c.bench_function("core/encode_tagged_to_value", |b| {
        b.iter(|| tagged_to_value(black_box(&tagged)));
    });
    c.bench_function("core/check_portable", |b| {
        b.iter(|| check_portable(black_box(&doc)).expect("document is portable"));
    });
    c.bench_function("core/nominal_ancestry", |b| {
        b.iter(|| has_ancestor(black_box(&child), black_box(&base)));
    });
}

criterion_group!(core_benches, bench_core);
criterion_main!(core_benches);
