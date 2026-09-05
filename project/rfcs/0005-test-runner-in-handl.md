# RFC-0005: Test Runner in Handl (`handl-packages/test-runner`)

## Status

Low priority. Ports the Rust test runner (`plugin-engine-test`,
[ADR-0089](../adrs/0089-bootstrap-test-runner.md)) to handl under
`handl-packages/test-runner/`. Per ADR-0085 rule 6, the Rust substrate
remains the bootstrap/fallback after the port.

This RFC is **only about the runner** (discovery, planning, scheduling,
execution, reporting). The test **framework library** (`assert`/`property`/
… primitives) is already handl per
[ADR-0092](../adrs/0092-test-framework-in-handl.md); it is a dependency of
the runner, not this RFC's subject.

## Summary

Port the ADR-0089 runner from Rust (`plugin-engine-test`) to handl under
`handl-packages/test-runner/`. The Rust runner is already
production-architected — providers → discovery → index/cache → filter →
planner → scheduler → engine → result store → summarizer → reporter, with
stable core types, an event model, an explicit capability model,
structured failures, deterministic ordering, and versioned contracts. The
port translates each stage into arrows over test-plan values, preserving
the contracts so CI consumers, predicates, providers, and reporters carry
over unchanged. Execution stays delegated to the runtime handler; parallel
execution maps onto ADR-0072's concurrent arrows.

## Detailed design

- **Test plan as data:** a `TestPlan` is a handl value (list of
  `{ module, name, description, metadata }`). Discovery is an arrow
  `Paths ~> TestIndex`; filtering is a pure arrow `TestIndex ~> TestPlan`
  (so `--filter` is just composition with a `TestPredicate`).
- **Stages as arrows:** each ADR-0089 pipeline stage
  (discovery/index/filter/planner/scheduler/engine/result-store/summarizer/
  reporter) becomes an arrow. The stage contracts and stable core types
  (`Test`, `TestPlan`, `ExecutionPlan`, `TestResult`, `TestReport`,
  `TestPredicate`, …) are preserved verbatim so providers, predicates, and
  reporters port without breaking consumers.
- **Test-language fragment:** unchanged from ADR-0089 — the
  `test`/`property`/`integration_test`/`bench` keyword forms remain a
  mergeable language fragment (ADR-0044), not core grammar. The handl
  runner merges it onto the canonical kit the same way the Rust runner
  does (`Language::merge`).
- **Execution via handler dispatch:** running a test is a capability
  request to the runtime handler; a local handler runs it in-process, a
  distributed handler can shard the plan across workers. The runner never
  interprets assertions itself — it calls the framework library
  (`assert.*` etc., already handl per ADR-0092).
- **Parallel execution** maps onto ADR-0072's concurrent arrows:
  `test-plan >>> fan-out >>> collect-results`, with per-test isolation
  guaranteed because handl values are immutable. Determinism is preserved
  (ADR-0089 invariant): results re-collect into canonical order before
  reporting regardless of completion order.
- **Pure engine by default; capability model unchanged.** The handl runner
  honors the same capability reasoning as the Rust runner — it queries the
  (merged) test language's capability set (ADR-0044 §7, a language-level
  concept) and maps it to scheduling decisions. The runner does not define
  a capability enum. Future engines (`SandboxEngine`, `ProcessEngine`,
  `RemoteEngine`) implement the same `TestEngine` contract.
- **Generic systems port as generic systems.** The Rust runner consumes
  generic platform systems (capability reasoning, eval-result memoisation,
  bounded-job scheduler, predicate combinators, event bus — see ADR-0089
  _Generic infrastructure dependencies_). The handl port consumes the
  handl equivalents of those same generic systems; it does not fold them
  into the runner. The runner ports only its test-specific glue
  (providers, planner policy, test event vocabulary, reporters).
- **Properties and fuzzing:** already provided in ADR-0089 by the
  `PropertyProvider`; the port keeps `property "name" { ... }` and supplies
  shrinking via stdlib generators. Benchmarks (`bench "name"`) are
  collected but not run under `hnl test`.
- **Reporting:** unchanged contract from ADR-0089 (PASS/FAIL/SKIP lines,
  `--json`, `--report junit`, exit codes) so CI consumers don't change
  during migration.

## Migration

1. Port the runner's own conformance tests; run both runners on the stdlib
   suite until reports are identical.
2. Flip `hnl test` to the handl runner under `handl-packages/test-runner/`.
3. Retire `crates/plugin-engine-test` as the primary (Rust version remains
   the fallback per ADR-0085 rule 6).

## Drawbacks

- Concurrency semantics of the eval engine must be stable (ADR-0072) before
  parallel runs are trustworthy.
- The test-language fragment's `AstShapeDef` roles are a load-bearing
  contract shared by both runners during migration; changing them is a
  coordinated edit across Rust and handl.

## Related

- [ADR-0089](../adrs/0089-bootstrap-test-runner.md) — the Rust runner being
  ported.
- [ADR-0092](../adrs/0092-test-framework-in-handl.md) — the test framework
  library in handl (a dependency, not this RFC's subject).
- ADR-0072 (concurrent arrow calculus), ADR-0044 (language as data),
  [RFC-0007](0007-cli-in-handl.md).
