# ADR-0092: Test Framework in Handl (`handl-packages/test`), Delete `plugin-lib-test`

## Status

Implemented. `plugin-lib-test` is **deleted**. The framework contract
remains in `handl-packages/test/test.hnl` (`op`/`handl` declarations), and
all 12 temporary Rust handls have been replaced by pure-handl modules under
`handl-packages/test/`:

- `assert.hnl` — `assert.equal`, `assert.not_equal`, `assert.true`,
  `assert.false`, `assert.contains`
- `snapshot.hnl` — `snapshot.format`, `snapshot.match`
- `property.hnl` — `property.for_all`
- `fuzz.hnl` — `fuzz.run`
- `benchmark.hnl` — `benchmark.run`
- `test.hnl` — `test.run` (suite executor)

No Rust handls remain in the framework. `plugin-engine-test::framework`
retains only a thread-local assert-failure side-channel so the runner can
capture structured `expected`/`actual`/`diff` detail from the tree-walk
`assert` keyword handler.

Part of the bootstrap tooling strategy
([ADR-0085](./0085-bootstrap-tooling-strategy.md)) and an instance of the
stdlib-migration program ([ADR-0077](./0077-stdlib-migration-to-handl.md)).
Companion to [ADR-0089](./0089-bootstrap-test-runner.md), which extracts
the test _runner_ into a new `plugin-engine-test` crate. The future handl
port of the runner is [RFC-0005](../rfcs/0005-test-runner-in-handl.md).

> **Terminology.** This ADR is about the test **framework library** — the
> `assert`/`property`/`benchmark`/`fuzz`/`snapshot`/`test` primitives that
> handl test code _calls_. It is **not** about the test _runner_ (discovery,
> planning, scheduling, reporting), which is ADR-0089 and stays Rust for
> now.

## Context

Today `plugin-lib-test` is one crate hosting **two unrelated concerns**:

1. **The framework library** — `assert.rs`, `property.rs`, `benchmark.rs`,
   `fuzz.rs`, `snapshot.rs`, `test.rs`, plus the `test.hnl` op/handl
   wiring (`OpRegistry`, `HandlModule`s, `TestLibraryPlugin`). This is the
   handl-callable API: `assert.equal`, `property.for_all`,
   `snapshot.match`, etc.
2. **The runner** — `runner.rs` (discovery, execution, reporting).

ADR-0089 extracts the runner into `plugin-engine-test`. This ADR migrates
the **framework library** to handl under `handl-packages/test/` and
**deletes `plugin-lib-test`**.

The framework is already _declared_ in handl: `handl-packages/test/test.hnl`
declares the `op` interface (`assert.equal : (Value, Value) -> Bool`, …) and
binds each op to a Rust handl (`handl "assert.equal" = rust_assert_equal`).
The Rust modules implement those handls and register them in an
`OpRegistry`. This is exactly the Phase 1 `op`/`handl` form ADR-0077
describes for the stdlib migration: the contract is handl, the
implementation is Rust. ADR-0077's program is to replace the Rust handls
with pure-handl implementations and drop the Rust bridge.

The framework library is **pure logic** in almost all cases:

- `assert.*` — value equality, containment, boolean checks, thrown-error
  capture. All expressible in handl over `Value`.
- `snapshot.format` — pretty-print a value to JSON. Expressible in handl
  once a JSON serializer exists in the stdlib.
- `property.for_all` — run a function with a seeded RNG for N iterations.
  Needs a deterministic RNG primitive; the iteration/compare logic is
  pure handl.
- `benchmark.run` — timing. **Not pure**: needs a clock primitive. Stays a
  thin FFI handl until a `time.now` primitive lands in the stdlib, then
  moves to handl.
- `fuzz.run` — like `property.for_all` but failure-collecting. Same RNG
  dependency.
- `test.run` — run a suite and return a report. Pure handl over the
  result types.

So the migration is almost entirely pure-handl; the only FFI dependencies
are a seeded RNG primitive (for `property`/`fuzz`) and a clock primitive
(for `benchmark`). Both are stdlib concerns, not framework concerns.

## Decision

Migrate the test framework library from `plugin-lib-test` (Rust) to
**pure handl under `handl-packages/test/`**, following the ADR-0077
`op`/`handl` → pure-handl program. Then **delete `plugin-lib-test`**.

### Target layout

```
handl-packages/test/
    test.hnl         -- op declarations + cap declarations (already exists)
    assert.hnl       -- pure-handl assert.* implementations
    property.hnl     -- pure-handl property.for_all (over an RNG primitive)
    fuzz.hnl         -- pure-handl fuzz.run (over an RNG primitive)
    snapshot.hnl     -- pure-handl snapshot.format / snapshot.match
    benchmark.hnl    -- handl over a clock primitive until time.* lands
    test.hnl         -- pure-handl test.run (suite → report)
    docs/            -- (already exists)
```

Each `op` keeps its declared signature; only the `handl "..." = rust_*`
binding is replaced by a pure-handl definition (or, for `benchmark`/RNG
cases, by a handl to a stdlib FFI primitive).

### Migration per op

| Op                                                                      | Now        | After                 | FFI dependency                                |
| ----------------------------------------------------------------------- | ---------- | --------------------- | --------------------------------------------- |
| `assert.equal` / `not_equal` / `true` / `false` / `contains` / `throws` | Rust handl | pure handl            | none                                          |
| `snapshot.format` / `snapshot.match`                                    | Rust handl | pure handl            | a stdlib JSON serializer                      |
| `property.for_all`                                                      | Rust handl | pure handl            | a seeded RNG primitive (`random.*` in stdlib) |
| `fuzz.run`                                                              | Rust handl | pure handl            | same seeded RNG primitive                     |
| `test.run`                                                              | Rust handl | pure handl            | none                                          |
| `benchmark.run`                                                         | Rust handl | handl over `time.now` | a clock primitive (`time.*` in stdlib)        |

Ops with an FFI dependency land as handl-over-primitive the moment the
primitive exists; until then they stay as Rust handls _in the runner crate_
(`plugin-engine-test`) as a temporary bridge, not in a resurrected
`plugin-lib-test`.

### Crate boundary

| Crate                           | Before                       | After                                                                                         |
| ------------------------------- | ---------------------------- | --------------------------------------------------------------------------------------------- |
| `plugin-lib-test`               | Framework library + runner   | **Deleted**                                                                                   |
| `plugin-engine-test` (ADR-0089) | —                            | Runner only; may carry temporary Rust handls for `benchmark`/RNG until stdlib primitives land |
| `handl-packages/test/`          | `test.hnl` (op declarations) | Full pure-handl framework library                                                             |

### Relationship to ADR-0089

ADR-0089 extracts the runner into `plugin-engine-test` **first**. This ADR
then migrates the framework library to handl and deletes `plugin-lib-test`.
The ordering matters: the runner must have a home before the framework's
old home is removed. The runner consumes the framework via the handl module
chain regardless of whether the framework is the Rust or the handl
implementation, so the migration is transparent to the runner.

### Relationship to ADR-0077

This is a direct instance of ADR-0077's stdlib-migration program
(`op`/`handl` Phase 1 → pure-handl). The test framework is a stdlib
library like any other; it is not special. The only reason it has its own
ADR is the coupled crate deletion (`plugin-lib-test`) and the FFI
dependencies (RNG, clock) that gate two of the ops.

### What stays Rust

- The **runner** (`plugin-engine-test`) — ADR-0089, stays Rust per
  ADR-0085 rule 6.
- **Temporary Rust handls** for `benchmark.run` (clock) and
  `property.for_all`/`fuzz.run` (seeded RNG) _if_ those stdlib primitives
  don't exist yet. These live in `plugin-engine-test` as stopgaps, **not**
  in a recreated `plugin-lib-test`. They are deleted the moment the
  primitives land.

## Consequences

- **`plugin-lib-test` is deleted.** The runner-framework tangle is gone.
  The runner is `plugin-engine-test` (Rust); the framework is
  `handl-packages/test/` (handl).
- **The test framework is written in handl, from day one of the
  migration.** This matches ADR-0077's direction and means the framework
  is type-checked, linted, and formatted by the same tooling as all other
  handl code.
- **Two ops gate on stdlib primitives** (seeded RNG, clock). Until those
  land, `benchmark`/`property`/`fuzz` use temporary Rust handls hosted in
  `plugin-engine-test`. This is an explicit, tracked stopgap — not a
  permanent split.
- **The `op` signatures in `test.hnl` are the stable contract.** Migrating
  an op from Rust-handl to pure-handl changes only the `handl "..." = …`
  binding; callers see no difference.
- **No grammar change.** The framework is handl code using existing
  surface syntax; the `test` _keyword_ (test declarations) is a separate
  concern owned by ADR-0089's test-language fragment.

## Prerequisites

1. **ADR-0089 lands first** — `plugin-engine-test` exists and the runner
   is extracted out of `plugin-lib-test`.
2. **A stdlib JSON serializer** for `snapshot.format` (likely already
   present or near-term per ADR-0077).
3. **A seeded RNG primitive** in the stdlib (`random.*`) for
   `property.for_all` / `fuzz.run`.
4. **A clock primitive** in the stdlib (`time.*`) for `benchmark.run`.

Items 3 and 4 are now satisfied: `rand.seeded_*` RNG ops and
`time.instant_ns` are wired into the canonical module chain. The
`error.raise`/`error.try` primitives were not needed for the framework; the
pure-handl implementation uses the **result pattern** only
(`{"__tag__":"ok",...}` / `{"__tag__":"err",...}`).

## Status

Implemented (see top). ADR-0089 landed first; the runner lives in
`plugin-engine-test`. The pure-handl framework lives in
`handl-packages/test/` and is registered by `handl-kit-canonical` through the
generic `HandlModule` seam.

### Implementation addendum (migration pass 2)

- `plugin-lib-test` is deleted. The remaining temporary Rust handls in
  `crates/plugin-engine-test/src/framework/` were removed.
- All `op`/`handl` declarations in `handl-packages/test/test.hnl` now
  resolve to pure-handl implementations in `handl-packages/test/`
  (`assert.hnl`, `property.hnl`, `benchmark.hnl`, `fuzz.hnl`,
  `snapshot.hnl`, `test.hnl`).
- The framework uses only the result pattern for failures; `assert.*`
  helpers return `err`-tagged values directly. The runner converts those
  back into `TestOutcome::Failed` and, via the `assert` keyword handler,
  reads the thread-local assert-failure side-channel for structured
  `expected`/`actual`/`diff` detail.
- `benchmark.run` uses `time.instant_ns`; `property.for_all` and `fuzz.run`
  use `rand.seeded_float`. No `error.raise`/`error.try` is used by the
  framework.
