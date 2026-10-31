+++
id = "ADR-0089"
title = "ADR-0089: Bootstrap Test Runner (`plugin-engine-test`)"
status = "open"
priority = "medium"
+++

---
title: Bootstrap Test Runner (`plugin-engine-test`)
type: adr
date: 2026-10-15
updated: 2026-10-29
status: implemented
status_note: Implemented — the runner lives in the engine-test repo (`hnl-engine-test`); `plugin-ext-test` is the capability adapter.
refs: [ADR-0085, ADR-0092]
---

# ADR-0089: Bootstrap Test Runner (`plugin-engine-test`)

## Decision

1. **A new `plugin-engine-test` crate owns the runner** — discovery, index,
   filter, planner, scheduler, engine, result store, summarizer, reporter —
   extracted from `plugin-lib-test`; it stays Rust (ADR-0085 §6).
2. **`plugin-lib-test` becomes the framework library** (`assert`/`property`/
   `benchmark`/`fuzz`/`snapshot`), migrated to handl by ADR-0092 and then
   deleted.
3. **Stable pipeline contracts**: Providers → Discovery → Index/Cache → Filter
   → Planner → Scheduler → Engine → ResultStore → Summarizer → Reporter.
4. **Core types are hard to change and versioned**; internal contracts are
   versioned; failures are structured, never a bare `false`.
5. **Test declaration is a merged language fragment**, not the `test_*`
   naming convention the v0.1 grammar forced.
6. **Providers are extensible from day one**; discovery/index and filtering
   are predicate models.
7. **Planner and scheduler are separate; the scheduler is generic**; the
   engine is pure by default and the capability model is generic.
8. **Determinism is a hard invariant**; observability emits structured events
   from day one; engine genericity follows ADR-0081.

## Why

The original minimal runner discovered tests by the `test_*` naming workaround,
ran them in source order, and deferred parallelism, properties, benchmarks,
filtering, caching and providers to a future handl port. Bolting those on later
means rewriting the runner; the pipeline boundaries are the decision.

## Not doing

- **Ship the toy runner, bolt features on later** — the rewrite this ADR
  prevents.
- **Keep `test_*` naming** — a v0.1 grammar workaround; the test declaration
  is a language fragment.
- **One monolithic stage** — it welds discovery, scheduling and reporting
  together and makes any one of them unswappable.

## Consequences
- More structure up front (nine stage contracts) in exchange for additive
  features.
- The runner stays Rust while the framework moves to handl; the runner
  consumes the framework through the module chain either way.
- A temporary Rust handl may exist for `benchmark`/RNG until stdlib primitives
  land.

## Enforcement
- `engine-test/` (`hnl-engine-test`) — the runner.
- `crates/plugin-ext-test/` — the `test` capability adapter.
- `tests/` in the engine and host — pipeline and determinism tests.

## Design
+Stage contracts, providers, cache domains and the CLI/API surface:
+[`design/0089-bootstrap-test-runner.md`](../design/0089-bootstrap-test-runner.md).
