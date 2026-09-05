# ADR-0089: Bootstrap Test Runner (`plugin-lib-test`)

## Status

Implemented (merged to main). Part of the bootstrap tooling strategy
([ADR-0085](./0085-bootstrap-tooling-strategy.md)). Depends on
[ADR-0074](./0074-arrow-syntax-v0_1-surface-redesign.md) and the eval engine
(`plugin-engine-eval`). Temporary: will be replaced by the self-hosted test
runner described in [RFC-0005](../rfcs/0005-test-runner-in-handl.md).

## Context

The stdlib (ADR-0077, ADR-0083) will be written in handl and needs tests
that run in CI. `plugin-lib-test` predates the arrow surface and targets the
old value model; it will be brought up to the v0.1 surface in place, not
forked (ADR-0085 rule 1). A test runner coupled to the v0.1 surface is the
missing piece that makes "100% coverage" meaningful for handl code itself.

## Decision

The test-runner work lands in `plugin-lib-test` (plus its capability
wiring): it discovers, runs, and reports tests written in handl.

- **Test discovery:** handl source files may declare test blocks in the
  v0.1 surface (e.g. `test "name" { ... }` at module top level — exact
  syntax to be fixed against ADR-0074's grammar and desugaring document).
  The runner parses modules with `plugin-engine-parse`, extracts test
  declarations, and ignores everything else.
- **Execution:** each test is a description evaluated by the
  `plugin-engine-eval` engine (tree-walk first; bytecode transparently).
  Assertions come from the stdlib (`assert`, `assert.eq`, `assert.fail`)
  — the runner runs the module; failures surface as raised errors, which
  the runner catches and records. No runner-side assertion DSL.
- **Filtering and selection:** `hnl test [paths...] [--filter <pat>]
  [--list]`. Module path + test name identify a test.
- **Reporting:**
  - human output: one line per test (`PASS`/`FAIL`/`SKIP`), summary counts,
    failures printed with captured error message and test location (file +
    test name).
  - `--json` for CI consumers.
  - exit 1 iff any test fails or any module fails to parse.
- **Determinism:** tests run in source order per module; modules in
  lexicographic order; no shared state between tests (handl has no
  mutation primitive — this falls out of the language).
- **API surface:**

  ```rust
  pub fn discover(paths: &[PathBuf]) -> Result<TestPlan, DiscoverError>;
  pub fn run(plan: &TestPlan) -> RunReport;
  ```

- **Testing:** 100% coverage. The crate's own tests double as conformance
  tests for the `test` declaration syntax; the stdlib's test suites become
  integration fixtures for the runner.

Out of scope: fixtures, setup/teardown (no mutation to tear down),
properties/fuzzing, benchmarks, parallelism (all RFC-0005, where parallel
execution maps onto ADR-0072's concurrent arrows).

## Consequences

- Stdlib development is test-driven from the first handl commit.
- The `test` declaration becomes de-facto syntax — its exact shape must be
  pinned early and kept in sync with ADR-0074 grammar updates; the runner
  is the enforcer of that stability.
- Assertion logic living in the stdlib (not the runner) means the runner
  stays tiny and RFC-0005 can re-host it without behavior drift.

## Status

Implemented (merged to main).
